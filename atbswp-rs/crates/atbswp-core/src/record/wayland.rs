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

/// Merges the two Wayland sources on one CLOCK_MONOTONIC timeline: absolute
/// cursor samples from the screen-cast stream and raw key/button/wheel
/// events from evdev (in-process, a pkexec helper, or a test file).
struct Merger {
    b: Builder,
    cursor: Option<CursorStream>,
    pending: VecDeque<super::pw::Sample>,
    last_t: u64,
}

impl Merger {
    fn new(opts: &Options, cursor: Option<CursorStream>) -> Self {
        Merger {
            b: Builder::new(opts),
            cursor,
            pending: VecDeque::new(),
            last_t: 0,
        }
    }

    /// Emit cursor positions sampled up to time `t` as absolute moves.
    fn drain_cursor(&mut self, t: u64) {
        if let Some(cs) = &self.cursor {
            self.pending.extend(cs.samples.try_iter());
        }
        while self.pending.front().is_some_and(|s| s.t_us <= t) {
            let s = self.pending.pop_front().unwrap();
            self.b.move_abs(s.t_us, s.x, s.y);
        }
    }

    fn feed(&mut self, e: RawEvent) {
        self.drain_cursor(e.t_us);
        self.last_t = e.t_us;
        // with a cursor stream, evdev's relative motion is redundant
        evdev::apply(&mut self.b, &e, self.cursor.is_some());
    }

    /// Consume raw events in the helper's line format until `END`.
    fn feed_lines(&mut self, r: impl std::io::Read) -> Result<(), String> {
        for line in BufReader::new(r).lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line == "END" {
                break;
            }
            if let Some(e) = RawEvent::from_line(&line) {
                self.feed(e);
            }
        }
        Ok(())
    }

    fn finish(mut self, opts: &Options) -> Macro {
        // Motion after the last key/button still matters (e.g. a final hover).
        self.drain_cursor(self.last_t.max(super::monotonic_us()));
        let mut header = Header::default();
        if let Some(cs) = self.cursor.take() {
            if let Some(err) = cs.error() {
                eprintln!("atbswp: cursor stream reported: {err}");
            }
            if let Some((w, h)) = cs.size() {
                (header.screen_w, header.screen_h) = (w, h);
            }
            cs.stop();
        }
        if let Some((w, h)) = opts.screen {
            (header.screen_w, header.screen_h) = (w, h);
        }
        self.b.finish(header)
    }
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

/// Re-run the CLI recorder as root through pkexec; it streams raw events
/// back on stdout (see [`stream_raw_to_stdout`]).
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

fn start_cursor_stream() -> Option<CursorStream> {
    let result = portal::open_screencast().and_then(|sc| {
        let (w, h) = sc.stream.size.unwrap_or((0, 0));
        eprintln!("atbswp: screen-cast stream {} ({w}x{h})", sc.node_id);
        CursorStream::start(sc.fd, sc.node_id)
    });
    match result {
        Ok(cs) => Some(cs),
        Err(e) => {
            eprintln!(
                "atbswp: no cursor stream ({e}); pointer motion will be recorded as relative evdev motion"
            );
            None
        }
    }
}

fn evdev_error(e: evdev::Error) -> String {
    match e {
        evdev::Error::Permission => evdev::PERMISSION_HINT.to_string(),
        evdev::Error::Other(m) => m,
    }
}

pub fn record(opts: &Options) -> Result<Macro, String> {
    let mut merger = Merger::new(opts, start_cursor_stream());
    let stop_name = opts
        .stop_key
        .and_then(atbswp_macro::keys::key_name)
        .unwrap_or("Ctrl-C");
    eprintln!("atbswp: recording; press {stop_name} to stop");

    if let Some(path) = opts.raw_from.as_deref() {
        // test hook: raw events from a file/FIFO in the helper's format
        let f = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
        merger.feed_lines(f)?;
    } else {
        match evdev::open_probe() {
            Ok(()) => evdev::run(opts, |e| merger.feed(e)).map_err(evdev_error)?,
            Err(evdev::Error::Permission) if opts.allow_elevate => {
                let mut child = spawn_elevated(opts)?;
                let out = child
                    .stdout
                    .take()
                    .ok_or("no pipe from the elevated recorder")?;
                merger.feed_lines(out)?;
                match child.wait().map_err(|e| e.to_string())?.code() {
                    Some(0) => {}
                    Some(126) | Some(127) => {
                        return Err("authorisation was cancelled or refused".into());
                    }
                    other => return Err(format!("elevated recorder failed ({other:?})")),
                }
            }
            Err(e) => return Err(evdev_error(e)),
        }
    }
    Ok(merger.finish(opts))
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
    r.map_err(evdev_error)
}
