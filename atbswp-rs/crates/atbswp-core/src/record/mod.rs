//! Input recording, one backend per platform, all unprivileged where the
//! platform allows it:
//!
//! | platform | source                                   | privilege                  |
//! |----------|------------------------------------------|----------------------------|
//! | X11      | XRecord extension                        | none                       |
//! | Windows  | WH_KEYBOARD_LL / WH_MOUSE_LL hooks       | none                       |
//! | macOS    | listen-only CGEventTap                   | Input Monitoring prompt    |
//! | Wayland  | ScreenCast cursor metadata + evdev       | consent dialog + pkexec    |
//!
//! Wayland has no passive input-observation API: the InputCapture portal
//! (libei's receiver side) only delivers events after a pointer barrier is
//! crossed and takes them away from the desktop while it does, so it cannot
//! record a normal session.  evdev is the only option, and it needs root or
//! the `input` group; rather than a permanent group change we re-run the
//! recorder once through `pkexec`, which shows the desktop's password dialog.

pub mod builder;
pub mod keymap;
pub use builder::Builder;

#[cfg(target_os = "linux")]
mod dbus;
#[cfg(target_os = "linux")]
mod dl;
#[cfg(target_os = "linux")]
mod evdev;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod portal;
#[cfg(target_os = "linux")]
mod pw;
#[cfg(target_os = "linux")]
mod touchpad;
#[cfg(target_os = "linux")]
pub mod wayland;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

use atbswp_macro::Macro;
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
    /// Test hook: read raw evdev events (helper line format) from this file
    /// instead of /dev/input.
    pub raw_from: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            stop_key: Some(88), // KEY_F12
            screen: None,
            min_move_interval_us: 10_000,
            handle_signals: false,
            allow_elevate: true,
            raw_from: None,
        }
    }
}

static STOP: AtomicBool = AtomicBool::new(false);

/// Ask a running [`record`] call (on another thread) to finish.
pub fn request_stop() {
    STOP.store(true, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    wayland::stop_helper();
}

pub(crate) fn stop_requested() -> bool {
    STOP.load(Ordering::SeqCst)
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
        if std::env::var("ATBSWP_RECORDER").as_deref() == Ok("evdev") {
            return evdev::record(opts).map_err(|e| match e {
                evdev::Error::Permission => evdev::PERMISSION_HINT.to_string(),
                evdev::Error::Other(m) => m,
            });
        }
        wayland::record(opts)
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

/// Microseconds on CLOCK_MONOTONIC: the one clock every recorder source
/// (evdev, the pkexec helper, the PipeWire cursor stream) is stamped with.
#[cfg(target_os = "linux")]
pub(crate) fn monotonic_us() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1000
}
