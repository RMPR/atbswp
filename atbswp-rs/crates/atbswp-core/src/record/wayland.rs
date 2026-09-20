//! Wayland recorder: absolute cursor positions from a ScreenCast portal
//! stream (PipeWire cursor metadata), keys/buttons/scroll from evdev, either
//! in-process or through a `pkexec`-elevated helper streaming raw events.
//! Everything is merged on CLOCK_MONOTONIC.

use super::evdev::{self, RawEvent};
use super::pw::CursorStream;
use super::{Builder, Options, portal};
use atbswp_macro::{Header, Macro};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

/// Source of raw evdev events: in-process, a child running as root, or (for
/// tests) a file/FIFO of raw event lines in the helper's format.
enum Source {
    Local,
    Elevated(Child),
    Reader(Box<dyn std::io::Read>),
}

fn cli_binary() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if exe.file_name().is_some_and(|n| n == "atbswp") {
            return exe;
        }
        let sibling = exe.with_file_name("atbswp");
        if sibling.exists() {
            return sibling;
        }
    }
    std::path::PathBuf::from("atbswp")
}

fn spawn_elevated(opts: &Options) -> Result<Child, String> {
    let mut cmd = Command::new("pkexec");
    cmd.arg(cli_binary())
        .arg("record")
        .arg("--stdout-raw")
        .arg("--no-elevate");
    if let Some(k) = opts.stop_key {
        cmd.arg("--stop-key").arg(k.to_string());
    }
    eprintln!("atbswp: /dev/input is not readable; asking for authorisation via pkexec");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("pkexec: {e} (is polkit installed?)"))
}

/// Merges cursor samples (absolute) into the builder up to time `t`.
fn drain_cursor(
    b: &mut Builder,
    pending: &mut VecDeque<super::pw::Sample>,
    stream: Option<&CursorStream>,
    t: u64,
) {
    if let Some(cs) = stream {
        for s in cs.samples.try_iter() {
            pending.push_back(s);
        }
    }
    while let Some(s) = pending.front() {
        if s.t_us > t {
            break;
        }
        let s = pending.pop_front().unwrap();
        b.move_abs(s.t_us, s.x, s.y);
    }
}

pub fn record(opts: &Options) -> Result<Macro, String> {
    // 1. Cursor stream via the ScreenCast portal (absolute positions).
    let cursor = match portal::open_screencast().and_then(|sc| {
        eprintln!(
            "atbswp: screen-cast stream {} ({}x{})",
            sc.node_id,
            sc.stream.size.map_or(0, |s| s.0),
            sc.stream.size.map_or(0, |s| s.1)
        );
        CursorStream::start(sc.fd, sc.node_id)
    }) {
        Ok(cs) => Some(cs),
        Err(e) => {
            eprintln!(
                "atbswp: no cursor stream ({e}); pointer motion will be recorded as relative evdev motion"
            );
            None
        }
    };

    // 2. Raw events: locally if we can read /dev/input, else via pkexec.
    let source = match opts.raw_from.as_deref() {
        Some(path) => Source::Reader(Box::new(
            std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?,
        )),
        None => match evdev::open_probe() {
            Ok(()) => Source::Local,
            Err(evdev::Error::Permission) if opts.allow_elevate => {
                Source::Elevated(spawn_elevated(opts)?)
            }
            Err(evdev::Error::Permission) => return Err(evdev::PERMISSION_HINT.into()),
            Err(evdev::Error::Other(e)) => return Err(e),
        },
    };
    let stop_name = opts
        .stop_key
        .and_then(atbswp_macro::keys::key_name)
        .unwrap_or("Ctrl-C");
    eprintln!("atbswp: recording; press {stop_name} to stop");

    let mut b = Builder::new(opts);
    let mut pending = VecDeque::new();
    let have_cursor = cursor.is_some();
    let mut last_t = 0;
    let feed = |b: &mut Builder, pending: &mut VecDeque<_>, e: RawEvent| {
        drain_cursor(b, pending, cursor.as_ref(), e.t_us);
        evdev::apply(b, &e, have_cursor);
    };

    match source {
        Source::Reader(r) => {
            for line in BufReader::new(r).lines() {
                let line = line.map_err(|e| e.to_string())?;
                if line == "END" {
                    break;
                }
                if let Some(e) = RawEvent::from_line(&line) {
                    last_t = e.t_us;
                    feed(&mut b, &mut pending, e);
                }
            }
        }
        Source::Local => {
            evdev::run(opts, |e| {
                last_t = e.t_us;
                feed(&mut b, &mut pending, e);
            })
            .map_err(|e| match e {
                evdev::Error::Permission => evdev::PERMISSION_HINT.to_string(),
                evdev::Error::Other(m) => m,
            })?;
        }
        Source::Elevated(mut child) => {
            let out = child
                .stdout
                .take()
                .ok_or("no pipe from the elevated recorder")?;
            for line in BufReader::new(out).lines() {
                let line = line.map_err(|e| e.to_string())?;
                if line == "END" {
                    break;
                }
                if let Some(e) = RawEvent::from_line(&line) {
                    last_t = e.t_us;
                    feed(&mut b, &mut pending, e);
                }
            }
            let status = child.wait().map_err(|e| e.to_string())?;
            match status.code() {
                Some(0) => {}
                Some(126) | Some(127) => {
                    return Err("authorisation was cancelled or refused".into());
                }
                other => return Err(format!("elevated recorder failed ({other:?})")),
            }
        }
    }
    // Motion after the last key/button still matters (e.g. a final hover).
    drain_cursor(
        &mut b,
        &mut pending,
        cursor.as_ref(),
        last_t.max(evdev::monotonic_us()),
    );

    let mut header = Header {
        ..Default::default()
    };
    if let Some(cs) = cursor {
        if let Some(err) = cs.error() {
            eprintln!("atbswp: cursor stream reported: {err}");
        }
        if let Some((w, h)) = cs.size() {
            header.screen_w = w;
            header.screen_h = h;
        }
        cs.stop();
    }
    if let Some((w, h)) = opts.screen {
        header.screen_w = w;
        header.screen_h = h;
    }
    Ok(b.finish(header))
}

/// The elevated helper's job: raw events as lines on stdout.
pub fn stream_raw_to_stdout(opts: &Options) -> Result<(), String> {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let r = evdev::run(opts, |e| {
        let _ = writeln!(out, "{}", e.to_line());
        let _ = out.flush();
    });
    let _ = writeln!(out, "END");
    let _ = out.flush();
    r.map_err(|e| match e {
        evdev::Error::Permission => evdev::PERMISSION_HINT.to_string(),
        evdev::Error::Other(m) => m,
    })
}
