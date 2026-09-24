//! The XDG ScreenCast portal session that feeds the Wayland cursor stream,
//! plus the restore tokens that make later recordings silent.

use super::dbus::{Portal, Stream, Value};
use std::os::unix::io::RawFd;
use std::time::Duration;

/// Where portal restore tokens live.
pub fn token_path(name: &str) -> Option<std::path::PathBuf> {
    if let Some(s) = std::env::var_os("XDG_STATE_HOME").filter(|s| !s.is_empty()) {
        return Some(std::path::PathBuf::from(s).join(format!("atbswp-{name}-token")));
    }
    std::env::var_os("HOME").map(|h| {
        std::path::PathBuf::from(h)
            .join(".local/state")
            .join(format!("atbswp-{name}-token"))
    })
}

pub fn load_token(name: &str) -> Option<String> {
    let t = std::fs::read_to_string(token_path(name)?).ok()?;
    let t = t.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

pub fn save_token(name: &str, token: &str) {
    if let Some(p) = token_path(name) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, format!("{token}\n"));
    }
}

/// A ScreenCast session delivering the cursor position as stream metadata.
pub struct ScreenCast {
    pub fd: RawFd,
    pub node_id: u32,
    pub stream: Stream,
}

pub fn open_screencast() -> Result<ScreenCast, String> {
    const IFACE: &str = "org.freedesktop.portal.ScreenCast";
    const CURSOR_MODE_METADATA: u32 = 4;
    const SOURCE_MONITOR: u32 = 1;
    let mut p = Portal::connect()?;
    let session_token = format!("atbswp_sc{}", std::process::id());
    let (code, results) = p.request(
        IFACE,
        "CreateSession",
        None,
        false,
        &[("session_handle_token", Value::Str(session_token))],
        Duration::from_secs(10),
    )?;
    let session = match results.iter().find(|(k, _)| k == "session_handle") {
        Some((_, Value::Str(s))) if code == 0 => s.clone(),
        _ => return Err(format!("ScreenCast CreateSession failed (response {code})")),
    };
    let mut opts = vec![
        ("types", Value::U32(SOURCE_MONITOR)),
        ("cursor_mode", Value::U32(CURSOR_MODE_METADATA)),
        ("persist_mode", Value::U32(2)),
    ];
    if let Some(t) = load_token("screencast") {
        opts.push(("restore_token", Value::Str(t)));
    }
    let (code, _) = p.request(
        IFACE,
        "SelectSources",
        Some(&session),
        false,
        &opts,
        Duration::from_secs(10),
    )?;
    if code != 0 {
        return Err(format!(
            "ScreenCast SelectSources failed (response {code}); does the compositor support cursor metadata?"
        ));
    }
    let (code, results) = p.request(
        IFACE,
        "Start",
        Some(&session),
        true,
        &[],
        Duration::from_secs(300),
    )?;
    if code != 0 {
        return Err(if code == 1 {
            "screen capture was cancelled".into()
        } else {
            format!("ScreenCast Start failed (response {code})")
        });
    }
    if let Some((_, Value::Str(t))) = results.iter().find(|(k, _)| k == "restore_token") {
        save_token("screencast", t);
    }
    let stream = match results.iter().find(|(k, _)| k == "streams") {
        Some((_, Value::Streams(s))) if !s.is_empty() => s[0].clone(),
        _ => return Err("ScreenCast Start returned no streams".into()),
    };
    let fd = p.call_for_fd(IFACE, "OpenPipeWireRemote", &session)?;
    Ok(ScreenCast {
        fd,
        node_id: stream.node_id,
        stream,
    })
}
