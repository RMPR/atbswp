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

pub fn monotonic_us() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1000
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
                devs.push(Device {
                    file,
                    buf: vec![],
                    wheel: 0,
                    hwheel: 0,
                    wheel_hi: 0,
                    hwheel_hi: 0,
                    has_hi_res: false,
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

    'outer: while !super::stop_requested() {
        let n = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 100) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(Error::Other(format!("poll: {err}")));
        }
        if n == 0 {
            continue;
        }
        for (i, pfd) in pollfds.iter_mut().enumerate() {
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            let dev = &mut devs[i];
            let mut chunk = [0u8; INPUT_EVENT_LEN * 64];
            let got = match dev.file.read(&mut chunk) {
                Ok(0) => continue,
                Ok(n) => n,
                Err(_) => continue, // would block, or device went away
            };
            dev.buf.extend_from_slice(&chunk[..got]);
            let mut consumed = 0;
            while dev.buf.len() - consumed >= INPUT_EVENT_LEN {
                let e = &dev.buf[consumed..consumed + INPUT_EVENT_LEN];
                consumed += INPUT_EVENT_LEN;
                let sec = i64::from_le_bytes(e[0..8].try_into().unwrap());
                let usec = i64::from_le_bytes(e[8..16].try_into().unwrap());
                let t_us = if sec > 0 {
                    (sec as u64) * 1_000_000 + usec as u64
                } else {
                    monotonic_us()
                };
                let typ = u16::from_le_bytes([e[16], e[17]]);
                let code = u16::from_le_bytes([e[18], e[19]]);
                let value = i32::from_le_bytes(e[20..24].try_into().unwrap());
                match typ {
                    EV_KEY => {
                        if value == 2 {
                            continue; // autorepeat
                        }
                        let pressed = value != 0;
                        if (BTN_MOUSE_FIRST..=BTN_MOUSE_LAST).contains(&code) {
                            sink(RawEvent {
                                t_us,
                                ev: Raw::Button { code, pressed },
                            });
                        } else if code < KEY_MAX {
                            if Some(code) == opts.stop_key {
                                if pressed {
                                    break 'outer;
                                }
                                continue;
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
                        REL_WHEEL => dev.wheel += value,
                        REL_HWHEEL => dev.hwheel += value,
                        REL_WHEEL_HI_RES => {
                            dev.wheel_hi += value;
                            dev.has_hi_res = true;
                        }
                        REL_HWHEEL_HI_RES => {
                            dev.hwheel_hi += value;
                            dev.has_hi_res = true;
                        }
                        _ => {}
                    },
                    EV_SYN => {
                        // Frame boundary: emit scroll. Kernel sign: +wheel = up.
                        let (sx, sy) = if dev.has_hi_res {
                            (dev.hwheel_hi, -dev.wheel_hi)
                        } else {
                            (dev.hwheel * 120, -dev.wheel * 120)
                        };
                        if sx != 0 || sy != 0 {
                            sink(RawEvent {
                                t_us,
                                ev: Raw::Scroll { x: sx, y: sy },
                            });
                        }
                        dev.wheel = 0;
                        dev.hwheel = 0;
                        dev.wheel_hi = 0;
                        dev.hwheel_hi = 0;
                    }
                    _ => {}
                }
            }
            dev.buf.drain(..consumed);
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
