//! Linux evdev recorder (Wayland fallback): reads `/dev/input/event*`.
//! Needs read access to those devices; see `elevated` in mod.rs.
//! Mouse motion is relative, since evdev cannot see the compositor's cursor.

use super::{Builder, Options};
use atbswp_macro::{Header, Macro, keys};
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::Ordering;

pub enum Error {
    Permission,
    Other(String),
}

/// One raw input event with a CLOCK_MONOTONIC timestamp, so events from
/// different processes (the pkexec helper, the cursor stream) can be merged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raw {
    Key { code: u16, pressed: bool },
    Button { code: u16, pressed: bool },
    Rel { dx: i32, dy: i32 },
    Scroll { x: i32, y: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawEvent {
    pub t_us: u64,
    pub ev: Raw,
}

impl RawEvent {
    /// Line form used between the elevated helper and its parent.
    pub fn to_line(self) -> String {
        match self.ev {
            Raw::Key { code, pressed } => format!("K {} {} {}", self.t_us, code, pressed as u8),
            Raw::Button { code, pressed } => format!("B {} {} {}", self.t_us, code, pressed as u8),
            Raw::Rel { dx, dy } => format!("R {} {} {}", self.t_us, dx, dy),
            Raw::Scroll { x, y } => format!("S {} {} {}", self.t_us, x, y),
        }
    }

    pub fn from_line(line: &str) -> Option<RawEvent> {
        let mut it = line.split_whitespace();
        let kind = it.next()?;
        let t_us = it.next()?.parse().ok()?;
        let a: i64 = it.next()?.parse().ok()?;
        let b: i64 = it.next()?.parse().ok()?;
        let ev = match kind {
            "K" => Raw::Key {
                code: a as u16,
                pressed: b != 0,
            },
            "B" => Raw::Button {
                code: a as u16,
                pressed: b != 0,
            },
            "R" => Raw::Rel {
                dx: a as i32,
                dy: b as i32,
            },
            "S" => Raw::Scroll {
                x: a as i32,
                y: b as i32,
            },
            _ => return None,
        };
        Some(RawEvent { t_us, ev })
    }
}

// EVIOCSCLOCKID = _IOW('E', 0xa0, int)
const EVIOCSCLOCKID: libc::c_ulong = 0x4004_45a0;

pub const PERMISSION_HINT: &str = "no readable input devices under /dev/input: run as root, \
    allow the pkexec prompt, or add yourself to the `input` group";

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;
const REL_X: u16 = 0;
const REL_Y: u16 = 1;
const REL_HWHEEL: u16 = 6;
const REL_WHEEL: u16 = 8;
const REL_WHEEL_HI_RES: u16 = 11;
const REL_HWHEEL_HI_RES: u16 = 12;
const BTN_MOUSE_FIRST: u16 = 0x110;
const BTN_MOUSE_LAST: u16 = 0x117;
const INPUT_EVENT_LEN: usize = 24; // struct input_event on 64-bit
const KEY_MAX: u16 = 0x100;

extern "C" fn on_sigint(_: libc::c_int) {
    super::STOP.store(true, Ordering::SeqCst);
}

struct Device {
    file: File,
    buf: Vec<u8>,
    wheel: i32,
    hwheel: i32,
    wheel_hi: i32,
    hwheel_hi: i32,
    has_hi_res: bool,
    /// Present for touchpads: synthesises tap-to-click and clickfinger.
    touchpad: Option<super::touchpad::Touchpad>,
}

const EV_ABS: u16 = 3;

// EVIOCGBIT(EV_KEY, len) = _IOC(_IOC_READ, 'E', 0x20 + EV_KEY, len)
fn eviocgbit_key(len: usize) -> libc::c_ulong {
    (2u64 << 30 | (len as u64) << 16 | (b'E' as u64) << 8 | 0x21) as libc::c_ulong
}
// EVIOCGABS(abs) = _IOR('E', 0x40 + abs, struct input_absinfo)  (24 bytes)
fn eviocgabs(abs: u16) -> libc::c_ulong {
    (2u64 << 30 | 24u64 << 16 | (b'E' as u64) << 8 | (0x40 + abs as u64)) as libc::c_ulong
}

/// Detect a touchpad (reports BTN_TOOL_FINGER) and size its motion threshold.
fn probe_touchpad(file: &File) -> Option<super::touchpad::Touchpad> {
    let mut bits = [0u8; 0x300 / 8];
    let r = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            eviocgbit_key(bits.len()) as _,
            bits.as_mut_ptr(),
        )
    };
    if r < 0 {
        return None;
    }
    let has = |code: u16| bits[(code / 8) as usize] & (1 << (code % 8)) != 0;
    if !has(super::touchpad::BTN_TOOL_FINGER) || !has(super::touchpad::BTN_TOUCH) {
        return None;
    }
    #[repr(C)]
    struct AbsInfo {
        value: i32,
        minimum: i32,
        maximum: i32,
        fuzz: i32,
        flat: i32,
        resolution: i32,
    }
    let mut info = AbsInfo {
        value: 0,
        minimum: 0,
        maximum: 0,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    };
    let r = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            eviocgabs(super::touchpad::ABS_X) as _,
            &mut info,
        )
    };
    let (res, range) = if r < 0 {
        (0, 1000)
    } else {
        (info.resolution, (info.maximum - info.minimum).max(1))
    };
    Some(super::touchpad::Touchpad::new(res, range))
}

fn open_devices() -> Result<Vec<Device>, Error> {
    let mut devs = vec![];
    let mut denied = 0;
    let entries =
        fs::read_dir("/dev/input").map_err(|e| Error::Other(format!("/dev/input: {e}")))?;
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("event") {
            continue;
        }
        match fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(entry.path())
        {
            Ok(file) => {
                // timestamps on CLOCK_MONOTONIC so they line up with other sources
                let clock: libc::c_int = libc::CLOCK_MONOTONIC;
                unsafe { libc::ioctl(file.as_raw_fd(), EVIOCSCLOCKID as _, &clock) };
                let touchpad = probe_touchpad(&file);
                devs.push(Device {
                    file,
                    buf: vec![],
                    wheel: 0,
                    hwheel: 0,
                    wheel_hi: 0,
                    hwheel_hi: 0,
                    has_hi_res: false,
                    touchpad,
                })
            }
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => denied += 1,
            Err(_) => {}
        }
    }
    if devs.is_empty() {
        return Err(if denied > 0 {
            Error::Permission
        } else {
            Error::Other("no input devices found under /dev/input".into())
        });
    }
    Ok(devs)
}

/// Read raw events from all devices until the stop key is pressed or
/// [`super::request_stop`] is called.  `sink` receives every event; the
/// stop key itself is not delivered.
/// What a device's event asked the loop to do.
enum Flow {
    Continue,
    Stop,
}

impl Device {
    /// Handle one 24-byte `input_event`, emitting raw events to `sink`.
    fn handle(&mut self, opts: &Options, e: &[u8], sink: &mut impl FnMut(RawEvent)) -> Flow {
        let sec = i64::from_le_bytes(e[0..8].try_into().unwrap());
        let usec = i64::from_le_bytes(e[8..16].try_into().unwrap());
        let t_us = if sec > 0 {
            (sec as u64) * 1_000_000 + usec as u64
        } else {
            super::monotonic_us()
        };
        let typ = u16::from_le_bytes([e[16], e[17]]);
        let code = u16::from_le_bytes([e[18], e[19]]);
        let value = i32::from_le_bytes(e[20..24].try_into().unwrap());
        match typ {
            EV_KEY => {
                if value == 2 {
                    return Flow::Continue; // autorepeat
                }
                let pressed = value != 0;
                if let Some(tp) = self.touchpad.as_mut()
                    && (code == BTN_MOUSE_FIRST || super::touchpad::Touchpad::is_tool_key(code))
                {
                    tp.key(code, pressed); // decided at the end of the frame
                } else if (BTN_MOUSE_FIRST..=BTN_MOUSE_LAST).contains(&code) {
                    sink(RawEvent {
                        t_us,
                        ev: Raw::Button { code, pressed },
                    });
                } else if code < KEY_MAX {
                    if Some(code) == opts.stop_key {
                        return if pressed { Flow::Stop } else { Flow::Continue };
                    }
                    sink(RawEvent {
                        t_us,
                        ev: Raw::Key { code, pressed },
                    });
                }
            }
            EV_REL => match code {
                REL_X => sink(RawEvent {
                    t_us,
                    ev: Raw::Rel { dx: value, dy: 0 },
                }),
                REL_Y => sink(RawEvent {
                    t_us,
                    ev: Raw::Rel { dx: 0, dy: value },
                }),
                REL_WHEEL => self.wheel += value,
                REL_HWHEEL => self.hwheel += value,
                REL_WHEEL_HI_RES => {
                    self.wheel_hi += value;
                    self.has_hi_res = true;
                }
                REL_HWHEEL_HI_RES => {
                    self.hwheel_hi += value;
                    self.has_hi_res = true;
                }
                _ => {}
            },
            EV_ABS => {
                if let Some(tp) = self.touchpad.as_mut() {
                    tp.abs(code, value);
                }
            }
            EV_SYN => self.end_of_frame(t_us, sink),
            _ => {}
        }
        Flow::Continue
    }

    /// Frame boundary: touchpad clicks and the accumulated wheel movement.
    fn end_of_frame(&mut self, t_us: u64, sink: &mut impl FnMut(RawEvent)) {
        if let Some(tp) = self.touchpad.as_mut() {
            for out in tp.frame(t_us) {
                let (code, pressed) = match out {
                    super::touchpad::Out::Press(c) => (c, true),
                    super::touchpad::Out::Release(c) => (c, false),
                };
                sink(RawEvent {
                    t_us,
                    ev: Raw::Button { code, pressed },
                });
            }
        }
        // Kernel sign: +wheel = up; ours: +y = down. Hi-res wins when present.
        let (sx, sy) = if self.has_hi_res {
            (self.hwheel_hi, -self.wheel_hi)
        } else {
            (self.hwheel * 120, -self.wheel * 120)
        };
        if sx != 0 || sy != 0 {
            sink(RawEvent {
                t_us,
                ev: Raw::Scroll { x: sx, y: sy },
            });
        }
        self.wheel = 0;
        self.hwheel = 0;
        self.wheel_hi = 0;
        self.hwheel_hi = 0;
    }
}

/// Read raw events from all devices until the stop key is pressed or
/// [`super::request_stop`] is called.  `sink` receives every event; the
/// stop key itself is not delivered.
pub fn run(opts: &Options, mut sink: impl FnMut(RawEvent)) -> Result<(), Error> {
    let mut devs = open_devices()?;
    if opts.handle_signals {
        unsafe {
            libc::signal(libc::SIGINT, on_sigint as *const () as usize);
            libc::signal(libc::SIGTERM, on_sigint as *const () as usize);
        }
    }
    let mut pollfds: Vec<libc::pollfd> = devs
        .iter()
        .map(|d| libc::pollfd {
            fd: d.file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();

    while !super::stop_requested() {
        let n = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 100) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(Error::Other(format!("poll: {err}")));
        }
        for (dev, pfd) in devs.iter_mut().zip(pollfds.iter()) {
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            let mut chunk = [0u8; INPUT_EVENT_LEN * 64];
            let got = match dev.file.read(&mut chunk) {
                Ok(n) if n > 0 => n,
                _ => continue, // nothing, would block, or the device went away
            };
            dev.buf.extend_from_slice(&chunk[..got]);
            // take whole events out of the buffer, keep any partial tail
            let whole = dev.buf.len() / INPUT_EVENT_LEN * INPUT_EVENT_LEN;
            let events: Vec<u8> = dev.buf.drain(..whole).collect();
            for e in events.as_chunks::<INPUT_EVENT_LEN>().0 {
                if let Flow::Stop = dev.handle(opts, e, &mut sink) {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

/// Feed a raw event into a [`Builder`]; motion is relative here.
pub fn apply(b: &mut Builder, e: &RawEvent, ignore_motion: bool) {
    match e.ev {
        Raw::Key { code, pressed } => {
            b.key(e.t_us, code, pressed);
        }
        Raw::Button { code, pressed } => b.button(e.t_us, code, pressed),
        Raw::Rel { dx, dy } => {
            if !ignore_motion {
                b.motion(e.t_us, dx, dy)
            }
        }
        Raw::Scroll { x, y } => b.scroll(e.t_us, x, y),
    }
}

/// Standalone evdev recording: keys, buttons, scroll and relative motion.
/// Check whether /dev/input is readable without keeping devices open.
pub fn open_probe() -> Result<(), Error> {
    open_devices().map(|_| ())
}

pub fn record(opts: &Options) -> Result<Macro, Error> {
    let stop_name = opts.stop_key.and_then(keys::key_name).unwrap_or("Ctrl-C");
    eprintln!("atbswp: recording from evdev; press {stop_name} to stop");
    let mut b = Builder::new(opts);
    run(opts, |e| apply(&mut b, &e, false))?;
    let (screen_w, screen_h) = opts.screen.unwrap_or((0, 0));
    Ok(b.finish(Header {
        screen_w,
        screen_h,
        ..Default::default()
    }))
}
