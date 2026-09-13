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
use std::time::Instant;

pub enum Error {
    Permission,
    Other(String),
}

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
            Ok(file) => devs.push(Device {
                file,
                buf: vec![],
                wheel: 0,
                hwheel: 0,
                wheel_hi: 0,
                hwheel_hi: 0,
                has_hi_res: false,
            }),
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

pub fn record(opts: &Options) -> Result<Macro, Error> {
    let mut devs = open_devices()?;
    if opts.handle_signals {
        unsafe {
            libc::signal(libc::SIGINT, on_sigint as *const () as usize);
            libc::signal(libc::SIGTERM, on_sigint as *const () as usize);
        }
    }
    let stop_name = opts.stop_key.and_then(keys::key_name).unwrap_or("Ctrl-C");
    eprintln!(
        "atbswp: recording from {} evdev devices; press {stop_name} to stop",
        devs.len()
    );

    let t0 = Instant::now();
    let mut b = Builder::new(opts);
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
            let now_us = t0.elapsed().as_micros() as u64;
            let mut consumed = 0;
            while dev.buf.len() - consumed >= INPUT_EVENT_LEN {
                let e = &dev.buf[consumed..consumed + INPUT_EVENT_LEN];
                consumed += INPUT_EVENT_LEN;
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
                            b.button(now_us, code, pressed);
                        } else if code < 0x100 && b.key(now_us, code, pressed) {
                            break 'outer;
                        }
                    }
                    EV_REL => match code {
                        REL_X => b.motion(now_us, value, 0),
                        REL_Y => b.motion(now_us, 0, value),
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
                            b.scroll(now_us, sx, sy);
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
    let (screen_w, screen_h) = opts.screen.unwrap_or((0, 0));
    Ok(b.finish(Header {
        screen_w,
        screen_h,
        ..Default::default()
    }))
}
