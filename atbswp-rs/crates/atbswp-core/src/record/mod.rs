//! Input recording, one backend per platform, all unprivileged where the
//! platform allows it:
//!
//! | platform | source                                   | privilege                  |
//! |----------|------------------------------------------|----------------------------|
//! | X11      | XRecord extension                        | none                       |
//! | Windows  | WH_KEYBOARD_LL / WH_MOUSE_LL hooks       | none                       |
//! | macOS    | listen-only CGEventTap                   | Input Monitoring prompt    |
//! | Wayland  | evdev (`/dev/input`)                     | polkit prompt via pkexec   |
//!
//! Wayland has no passive input-observation API: the InputCapture portal
//! (libei's receiver side) only delivers events after a pointer barrier is
//! crossed and takes them away from the desktop while it does, so it cannot
//! record a normal session.  evdev is the only option, and it needs root or
//! the `input` group; rather than a permanent group change we re-run the
//! recorder once through `pkexec`, which shows the desktop's password dialog.

pub mod keymap;

#[cfg(target_os = "linux")]
mod evdev;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

use atbswp_macro::{Event, Header, Macro};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Options {
    /// Key that ends the recording (not recorded itself).
    pub stop_key: Option<u16>,
    /// Screen size to store in the header; `None` = ask the backend.
    pub screen: Option<(u32, u32)>,
    /// Coalesce motion into at most one event per this interval.
    pub min_move_interval_us: u32,
    /// Also stop on SIGINT/SIGTERM (CLI); a GUI uses [`request_stop`].
    pub handle_signals: bool,
    /// On Wayland, re-run through `pkexec` when /dev/input is not readable.
    pub allow_elevate: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            stop_key: Some(88), // KEY_F12
            screen: None,
            min_move_interval_us: 10_000,
            handle_signals: false,
            allow_elevate: true,
        }
    }
}

static STOP: AtomicBool = AtomicBool::new(false);

/// Ask a running [`record`] call (on another thread) to finish.
pub fn request_stop() {
    STOP.store(true, Ordering::SeqCst);
}

pub(crate) fn stop_requested() -> bool {
    STOP.load(Ordering::SeqCst)
}

/// Which backend [`record`] would use right now, for diagnostics.
pub fn backend_name() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if prefer_x11() { "x11" } else { "evdev" }
    }
    #[cfg(target_os = "windows")]
    {
        "windows-hooks"
    }
    #[cfg(target_os = "macos")]
    {
        "cgeventtap"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "none"
    }
}

#[cfg(target_os = "linux")]
fn prefer_x11() -> bool {
    if let Ok(v) = std::env::var("ATBSWP_RECORDER") {
        return v == "x11";
    }
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let display = std::env::var_os("DISPLAY").is_some();
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    session == "x11" || (display && !wayland && session != "wayland")
}

/// Record until the stop key, [`request_stop`], or (if enabled) SIGINT.
pub fn record(opts: &Options) -> Result<Macro, String> {
    STOP.store(false, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    {
        if prefer_x11() {
            return x11::record(opts);
        }
        match evdev::record(opts) {
            Err(evdev::Error::Permission) if opts.allow_elevate => elevated::record(opts),
            Err(evdev::Error::Permission) => Err(evdev::PERMISSION_HINT.into()),
            Err(evdev::Error::Other(e)) => Err(e),
            Ok(m) => Ok(m),
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows::record(opts)
    }
    #[cfg(target_os = "macos")]
    {
        macos::record(opts)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = opts;
        Err("recording is not implemented on this platform".into())
    }
}

#[cfg(target_os = "linux")]
mod elevated {
    //! Re-run the CLI recorder as root through pkexec and read the payload
    //! back over stdout.  Only the tiny recorder runs privileged; the file
    //! is written by the unprivileged parent.
    use super::Options;
    use atbswp_macro::Macro;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    fn cli_binary() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if exe.file_name().is_some_and(|n| n == "atbswp") {
                return exe;
            }
            let sibling = exe.with_file_name("atbswp");
            if sibling.exists() {
                return sibling;
            }
        }
        PathBuf::from("atbswp")
    }

    pub fn record(opts: &Options) -> Result<Macro, String> {
        let mut cmd = Command::new("pkexec");
        cmd.arg(cli_binary())
            .arg("record")
            .arg("--stdout-binary")
            .arg("--no-elevate");
        if let Some(k) = opts.stop_key {
            cmd.arg("--stop-key").arg(k.to_string());
        }
        if let Some((w, h)) = opts.screen {
            cmd.arg("--screen").arg(format!("{w}x{h}"));
        }
        cmd.arg("--min-move-interval")
            .arg((opts.min_move_interval_us / 1000).to_string());
        eprintln!("atbswp: /dev/input is not readable; asking for authorisation via pkexec");
        let out = cmd
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| format!("pkexec: {e} (is polkit installed?)"))?;
        match out.status.code() {
            Some(0) => Macro::decode(&out.stdout).map_err(|e| format!("elevated recorder: {e}")),
            Some(126) | Some(127) => Err("authorisation was cancelled or refused".into()),
            other => Err(format!("elevated recorder failed ({other:?})")),
        }
    }
}

/// Turns a stream of timestamped raw events into macro events, coalescing
/// motion and handling the stop key.  Platform independent, unit tested.
pub struct Builder {
    events: Vec<Event>,
    last_emit_us: u64,
    start_us: Option<u64>,
    min_move_interval_us: u64,
    stop_key: Option<u16>,
    pending_rel: Option<(u64, i32, i32)>,
    pending_abs: Option<(u64, u64, i32, i32)>, // first, last, x, y
}

impl Builder {
    pub fn new(opts: &Options) -> Self {
        Builder {
            events: vec![],
            last_emit_us: 0,
            start_us: None,
            min_move_interval_us: opts.min_move_interval_us as u64,
            stop_key: opts.stop_key,
            pending_rel: None,
            pending_abs: None,
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
        if let Some((t, dx, dy)) = self.pending_rel.take()
            && (dx != 0 || dy != 0)
        {
            let d = self.delay_for(t);
            self.events.push(Event::move_rel(dx, dy, d));
        }
        if let Some((_, last, x, y)) = self.pending_abs.take() {
            let d = self.delay_for(last);
            self.events.push(Event::move_abs(x, y, d));
        }
    }

    pub fn motion(&mut self, now_us: u64, dx: i32, dy: i32) {
        if let Some((t, ..)) = self.pending_rel
            && now_us.saturating_sub(t) >= self.min_move_interval_us
        {
            self.flush_motion();
        }
        match &mut self.pending_rel {
            Some((_, x, y)) => {
                *x += dx;
                *y += dy;
            }
            None => self.pending_rel = Some((now_us, dx, dy)),
        }
    }

    pub fn move_abs(&mut self, now_us: u64, x: i32, y: i32) {
        if let Some((first, ..)) = self.pending_abs
            && now_us.saturating_sub(first) >= self.min_move_interval_us
        {
            self.flush_motion();
        }
        match &mut self.pending_abs {
            Some((_, last, px, py)) => {
                *last = now_us;
                *px = x;
                *py = y;
            }
            None => self.pending_abs = Some((now_us, now_us, x, y)),
        }
    }

    pub fn button(&mut self, now_us: u64, code: u16, pressed: bool) {
        self.flush_motion();
        let d = self.delay_for(now_us);
        self.events.push(Event::button(code, pressed, d));
    }

    /// Returns true when this was the stop key (recording should end).
    pub fn key(&mut self, now_us: u64, code: u16, pressed: bool) -> bool {
        if Some(code) == self.stop_key {
            return pressed;
        }
        self.flush_motion();
        let d = self.delay_for(now_us);
        self.events.push(Event::key(code, pressed, d));
        false
    }

    pub fn scroll(&mut self, now_us: u64, x: i32, y: i32) {
        self.flush_motion();
        let d = self.delay_for(now_us);
        self.events.push(Event::scroll(x, y, d));
    }

    /// Finish: flush motion, release anything still held, attach the header.
    pub fn finish(mut self, header: Header) -> Macro {
        self.flush_motion();
        release_all(Macro {
            header,
            events: self.events,
        })
    }
}

/// Make sure every key/button pressed during the recording is released at
/// the end, so a macro never leaves modifiers stuck.
fn release_all(mut m: Macro) -> Macro {
    use atbswp_macro::EventType;
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

#[cfg(test)]
mod tests {
    use super::*;
    use atbswp_macro::EventType;

    fn opts() -> Options {
        Options {
            min_move_interval_us: 10_000,
            stop_key: Some(88),
            ..Default::default()
        }
    }

    #[test]
    fn coalesces_relative_motion_and_keeps_timing() {
        let mut b = Builder::new(&opts());
        b.motion(1_000, 1, 0);
        b.motion(2_000, 2, 0);
        b.motion(3_000, 0, 5);
        assert!(!b.key(20_000, 30, true));
        b.motion(21_000, 1, 1);
        b.motion(40_000, 1, 1); // > interval: flushes the previous one
        assert!(!b.key(50_000, 30, false));
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
    fn coalesces_absolute_motion_to_last_position() {
        let mut b = Builder::new(&opts());
        b.move_abs(0, 1, 1);
        b.move_abs(2_000, 5, 5);
        b.move_abs(4_000, 9, 9);
        b.button(6_000, 0x110, true);
        b.button(7_000, 0x110, false);
        let m = b.finish(Header::default());
        assert_eq!(m.events.len(), 3);
        assert_eq!(
            (
                m.events[0].kind,
                m.events[0].x,
                m.events[0].y,
                m.events[0].delay_us
            ),
            (EventType::MoveAbs, 9, 9, 0) // the clock starts at the first event
        );
        assert_eq!(m.events[1].delay_us, 2_000);
    }

    #[test]
    fn stop_key_is_not_recorded_and_held_keys_are_released() {
        let mut b = Builder::new(&opts());
        assert!(!b.key(0, 29, true)); // leftctrl held
        assert!(!b.key(10, 88, false)); // stray release of the stop key: ignored
        assert!(b.key(20, 88, true)); // stop
        let m = b.finish(Header::default());
        assert_eq!(m.events.len(), 2);
        assert_eq!(
            (m.events[1].kind, m.events[1].code),
            (EventType::KeyRelease, 29)
        );
    }

    #[test]
    fn zero_motion_is_dropped() {
        let mut b = Builder::new(&opts());
        b.motion(0, 0, 0);
        b.scroll(5, 0, 120);
        let m = b.finish(Header::default());
        assert_eq!(m.events.len(), 1);
        assert_eq!(m.events[0].kind, EventType::Scroll);
    }
}
