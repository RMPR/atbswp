//! Input recording.
//!
//! Linux: read raw events from every `/dev/input/event*` device.  This works
//! on Wayland and X11 alike without compositor support, at the price of two
//! limitations documented in the README: it needs `input` group access, and
//! mouse motion is captured as relative deltas (the compositor's absolute
//! cursor position is not visible from evdev).

use atbswp_macro::{Event, Header, Macro};

pub struct Options {
    /// Key that ends the recording (not recorded itself).
    pub stop_key: Option<u16>,
    /// Also stop on SIGINT/SIGTERM (for the CLI; a GUI uses [`request_stop`]).
    pub handle_signals: bool,
    /// Screen size to store in the header (for scaling on playback).
    pub screen: Option<(u32, u32)>,
    /// Coalesce relative motion into at most one event per this interval.
    pub min_move_interval_us: u32,
}

use std::sync::atomic::{AtomicBool, Ordering};

static STOP: AtomicBool = AtomicBool::new(false);

/// Ask a running [`record`] call (on another thread) to finish.
pub fn request_stop() {
    STOP.store(true, Ordering::SeqCst);
}

fn stop_requested() -> bool {
    STOP.load(Ordering::SeqCst)
}

#[cfg(target_os = "linux")]
pub fn record(opts: &Options) -> Result<Macro, String> {
    STOP.store(false, Ordering::SeqCst);
    linux::record(opts)
}

#[cfg(not(target_os = "linux"))]
pub fn record(_opts: &Options) -> Result<Macro, String> {
    Err("recording is only implemented on Linux so far".into())
}

/// Turns a stream of timestamped raw events into macro events, coalescing
/// motion and scroll.  Platform independent, so it is unit tested here.
pub struct Builder {
    events: Vec<Event>,
    last_emit_us: u64,
    start_us: Option<u64>,
    min_move_interval_us: u64,
    pending_dx: i32,
    pending_dy: i32,
    pending_since_us: Option<u64>,
}

impl Builder {
    pub fn new(min_move_interval_us: u32) -> Self {
        Builder {
            events: vec![],
            last_emit_us: 0,
            start_us: None,
            min_move_interval_us: min_move_interval_us as u64,
            pending_dx: 0,
            pending_dy: 0,
            pending_since_us: None,
        }
    }

    fn delay_for(&mut self, now_us: u64) -> u32 {
        let start = *self.start_us.get_or_insert(now_us);
        let base = if self.events.is_empty() {
            start
        } else {
            self.last_emit_us
        };
        let d = now_us.saturating_sub(base);
        self.last_emit_us = now_us;
        d.min(u32::MAX as u64) as u32
    }

    fn flush_motion(&mut self) {
        if let Some(t) = self.pending_since_us.take() {
            if self.pending_dx != 0 || self.pending_dy != 0 {
                let d = self.delay_for(t);
                self.events
                    .push(Event::move_rel(self.pending_dx, self.pending_dy, d));
            }
            self.pending_dx = 0;
            self.pending_dy = 0;
        }
    }

    pub fn motion(&mut self, now_us: u64, dx: i32, dy: i32) {
        match self.pending_since_us {
            Some(t) if now_us.saturating_sub(t) >= self.min_move_interval_us => {
                self.flush_motion();
                self.pending_since_us = Some(now_us);
            }
            Some(_) => {}
            None => self.pending_since_us = Some(now_us),
        }
        self.pending_dx += dx;
        self.pending_dy += dy;
    }

    #[allow(dead_code)] // for future backends that see absolute positions
    pub fn move_abs(&mut self, now_us: u64, x: i32, y: i32) {
        self.flush_motion();
        let d = self.delay_for(now_us);
        self.events.push(Event::move_abs(x, y, d));
    }

    pub fn button(&mut self, now_us: u64, code: u16, pressed: bool) {
        self.flush_motion();
        let d = self.delay_for(now_us);
        self.events.push(Event::button(code, pressed, d));
    }

    pub fn key(&mut self, now_us: u64, code: u16, pressed: bool) {
        self.flush_motion();
        let d = self.delay_for(now_us);
        self.events.push(Event::key(code, pressed, d));
    }

    pub fn scroll(&mut self, now_us: u64, x: i32, y: i32) {
        self.flush_motion();
        let d = self.delay_for(now_us);
        self.events.push(Event::scroll(x, y, d));
    }

    pub fn finish(mut self, header: Header) -> Macro {
        self.flush_motion();
        Macro {
            header,
            events: self.events,
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{Builder, Options};
    use atbswp_macro::{Header, Macro, keys};
    use std::fs::{self, File};
    use std::io::{self, Read};
    use std::os::unix::io::AsRawFd;
    use std::sync::atomic::Ordering;
    use std::time::Instant;

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
        // per-frame scroll accumulation, hi-res wins over legacy notches
        wheel: i32,
        hwheel: i32,
        wheel_hi: i32,
        hwheel_hi: i32,
        has_hi_res: bool,
    }

    fn open_devices() -> Result<Vec<Device>, String> {
        let mut devs = vec![];
        let mut denied = 0;
        let entries = fs::read_dir("/dev/input").map_err(|e| format!("/dev/input: {e}"))?;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("event") {
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
                "no readable input devices: add yourself to the `input` group \
                 (sudo usermod -aG input $USER, then log in again) or run as root"
                    .into()
            } else {
                "no input devices found under /dev/input".into()
            });
        }
        Ok(devs)
    }

    use std::os::unix::fs::OpenOptionsExt;

    pub fn record(opts: &Options) -> Result<Macro, String> {
        let mut devs = open_devices()?;
        if opts.handle_signals {
            unsafe {
                libc::signal(libc::SIGINT, on_sigint as *const () as usize);
                libc::signal(libc::SIGTERM, on_sigint as *const () as usize);
            }
        }
        let stop_name = opts.stop_key.and_then(keys::key_name).unwrap_or("Ctrl-C");
        eprintln!(
            "recording from {} devices; press {stop_name} or Ctrl-C to stop",
            devs.len()
        );

        let t0 = Instant::now();
        let mut b = Builder::new(opts.min_move_interval_us);
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
                return Err(format!("poll: {err}"));
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
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                    Err(_) => continue, // device went away
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
                            } else if code < 0x100 {
                                if Some(code) == opts.stop_key {
                                    if pressed {
                                        break 'outer;
                                    }
                                    continue;
                                }
                                b.key(now_us, code, pressed);
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
        let m = b.finish(Header {
            screen_w,
            screen_h,
            ..Default::default()
        });
        Ok(release_all(m))
    }

    /// Make sure every key/button pressed during the recording is released at
    /// the end, so a macro never leaves modifiers stuck.
    fn release_all(mut m: Macro) -> Macro {
        use atbswp_macro::{Event, EventType};
        let mut held: Vec<(EventType, u16)> = vec![];
        for e in &m.events {
            match e.kind {
                EventType::KeyPress | EventType::ButtonPress => {
                    if !held.iter().any(|(k, c)| *k == e.kind && *c == e.code) {
                        held.push((e.kind, e.code));
                    }
                }
                EventType::KeyRelease => {
                    held.retain(|(k, c)| !(*k == EventType::KeyPress && *c == e.code))
                }
                EventType::ButtonRelease => {
                    held.retain(|(k, c)| !(*k == EventType::ButtonPress && *c == e.code))
                }
                _ => {}
            }
        }
        for (k, c) in held.into_iter().rev() {
            m.events.push(match k {
                EventType::KeyPress => Event::key(c, false, 0),
                _ => Event::button(c, false, 0),
            });
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atbswp_macro::EventType;

    #[test]
    fn coalesces_motion_and_keeps_timing() {
        let mut b = Builder::new(10_000);
        b.motion(1_000, 1, 0);
        b.motion(2_000, 2, 0);
        b.motion(3_000, 0, 5);
        b.key(20_000, 30, true);
        b.motion(21_000, 1, 1);
        b.motion(40_000, 1, 1); // > interval: flushes the previous one
        b.key(50_000, 30, false);
        let m = b.finish(Header::default());
        let kinds: Vec<_> = m.events.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EventType::MoveRel,
                EventType::KeyPress,
                EventType::MoveRel,
                EventType::MoveRel,
                EventType::KeyRelease
            ]
        );
        assert_eq!(
            (m.events[0].x, m.events[0].y, m.events[0].delay_us),
            (3, 5, 0)
        );
        assert_eq!(m.events[1].delay_us, 19_000);
        assert_eq!(
            (m.events[2].x, m.events[2].y, m.events[2].delay_us),
            (1, 1, 1_000)
        );
        assert_eq!(
            (m.events[3].x, m.events[3].y, m.events[3].delay_us),
            (1, 1, 19_000)
        );
        assert_eq!(m.events[4].delay_us, 10_000);
        assert_eq!(m.duration_us(), 49_000);
    }

    #[test]
    fn zero_motion_is_dropped() {
        let mut b = Builder::new(10_000);
        b.motion(0, 0, 0);
        b.scroll(5, 0, 120);
        let m = b.finish(Header::default());
        assert_eq!(m.events.len(), 1);
        assert_eq!(m.events[0].kind, EventType::Scroll);
    }
}
