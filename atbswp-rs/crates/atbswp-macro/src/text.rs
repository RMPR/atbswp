//! Human-editable script form of a macro.
//!
//! ```text
//! # comment
//! screen 1920x1080
//! repeat 1
//! speed 100
//! move 100 200          # absolute, recorded-screen pixels
//! wait 50ms             # also: 300us, 1.5s, or a bare number of microseconds
//! click left            # buttondown + buttonup
//! buttondown right
//! buttonup right
//! key KEY_A             # keydown + keyup
//! keydown leftshift
//! keyup KEY_LEFTSHIFT
//! moverel -5 7
//! scroll 0 120          # 1/120 of a notch; +y = down, +x = right
//! scroll down 2         # whole notches: up/down/left/right N
//! ```

use crate::format::{Event, EventType, Macro, SCROLL_NOTCH};
use crate::keys;
use crate::{Error, Result};
use std::fmt::Write as _;

fn syntax(line: usize, msg: impl Into<String>) -> Error {
    Error::Syntax {
        line,
        msg: msg.into(),
    }
}

/// Parse a duration like `50ms`, `2s`, `300us`, or bare microseconds.
pub fn parse_duration_us(s: &str) -> Option<u32> {
    let (num, unit) = match s.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => s.split_at(i),
        None => (s, "us"),
    };
    let v: f64 = num.trim().parse().ok()?;
    let mult = match unit.trim() {
        "us" | "µs" => 1.0,
        "ms" => 1_000.0,
        "s" => 1_000_000.0,
        "m" | "min" => 60_000_000.0,
        _ => return None,
    };
    let us = v * mult;
    if !(0.0..=u32::MAX as f64).contains(&us) {
        return None;
    }
    Some(us.round() as u32)
}

fn fmt_duration(us: u32) -> String {
    if us.is_multiple_of(1_000_000) {
        format!("{}s", us / 1_000_000)
    } else if us.is_multiple_of(1_000) {
        format!("{}ms", us / 1_000)
    } else {
        format!("{us}us")
    }
}

/// Parse a text script.
pub fn parse(src: &str) -> Result<Macro> {
    let mut m = Macro::default();
    let mut pending_delay: u32 = 0;

    for (idx, raw) in src.lines().enumerate() {
        let lineno = idx + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap().to_ascii_lowercase();
        let args: Vec<&str> = parts.collect();
        let arg = |i: usize| -> Result<&str> {
            args.get(i)
                .copied()
                .ok_or_else(|| syntax(lineno, format!("`{cmd}` needs more arguments")))
        };
        let int = |i: usize| -> Result<i32> {
            arg(i)?
                .parse()
                .map_err(|_| syntax(lineno, format!("expected a number, got `{}`", args[i])))
        };
        let mut push = |e: Event| m.events.push(e);
        let take_delay = |d: &mut u32| std::mem::take(d);

        match cmd.as_str() {
            "screen" => {
                let a = arg(0)?;
                let (w, h) = a
                    .split_once(['x', 'X'])
                    .ok_or_else(|| syntax(lineno, "expected WIDTHxHEIGHT"))?;
                m.header.screen_w = w.parse().map_err(|_| syntax(lineno, "bad width"))?;
                m.header.screen_h = h.parse().map_err(|_| syntax(lineno, "bad height"))?;
            }
            "repeat" => {
                m.header.repeat = arg(0)?.parse().map_err(|_| {
                    syntax(lineno, "repeat expects a non-negative count (0 = forever)")
                })?;
            }
            "speed" => {
                let v: u32 = arg(0)?
                    .parse()
                    .map_err(|_| syntax(lineno, "speed expects a positive percentage"))?;
                if v == 0 {
                    return Err(syntax(lineno, "speed must be greater than 0"));
                }
                m.header.speed_percent = v;
            }
            "wait" | "sleep" | "delay" => {
                let d = parse_duration_us(arg(0)?)
                    .ok_or_else(|| syntax(lineno, format!("bad duration `{}`", args[0])))?;
                pending_delay = pending_delay.saturating_add(d);
            }
            "move" | "moveto" => {
                let (x, y) = (int(0)?, int(1)?);
                push(Event::move_abs(x, y, take_delay(&mut pending_delay)));
            }
            "moverel" => {
                let (x, y) = (int(0)?, int(1)?);
                push(Event::move_rel(x, y, take_delay(&mut pending_delay)));
            }
            "buttondown" | "buttonup" | "click" => {
                let code = keys::button_code(arg(0)?)
                    .ok_or_else(|| syntax(lineno, format!("unknown button `{}`", args[0])))?;
                let d = take_delay(&mut pending_delay);
                match cmd.as_str() {
                    "buttondown" => push(Event::button(code, true, d)),
                    "buttonup" => push(Event::button(code, false, d)),
                    _ => {
                        push(Event::button(code, true, d));
                        push(Event::button(code, false, 0));
                    }
                }
            }
            "keydown" | "keyup" | "key" => {
                let code = keys::key_code(arg(0)?)
                    .ok_or_else(|| syntax(lineno, format!("unknown key `{}`", args[0])))?;
                let d = take_delay(&mut pending_delay);
                match cmd.as_str() {
                    "keydown" => push(Event::key(code, true, d)),
                    "keyup" => push(Event::key(code, false, d)),
                    _ => {
                        push(Event::key(code, true, d));
                        push(Event::key(code, false, 0));
                    }
                }
            }
            "scroll" => {
                let first = arg(0)?;
                let notches = |i: usize| -> Result<i32> {
                    let n = int(i)? as i64 * SCROLL_NOTCH as i64;
                    i32::try_from(n).map_err(|_| syntax(lineno, "scroll amount is too large"))
                };
                let (x, y) = match first.to_ascii_lowercase().as_str() {
                    "up" => (0, -notches(1)?),
                    "down" => (0, notches(1)?),
                    "left" => (-notches(1)?, 0),
                    "right" => (notches(1)?, 0),
                    _ => (int(0)?, int(1)?),
                };
                push(Event::scroll(x, y, take_delay(&mut pending_delay)));
            }
            other => return Err(syntax(lineno, format!("unknown command `{other}`"))),
        }
    }
    if pending_delay > 0 {
        // A trailing wait is meaningful for repeats: keep it as a no-op move.
        m.events.push(Event::move_rel(0, 0, pending_delay));
    }
    Ok(m)
}

/// Render a macro as a text script that [`parse`] accepts.
pub fn render(m: &Macro) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# atbswp macro: {} events, {:.3}s",
        m.events.len(),
        m.duration_us() as f64 / 1e6
    );
    if m.header.screen_w != 0 || m.header.screen_h != 0 {
        let _ = writeln!(s, "screen {}x{}", m.header.screen_w, m.header.screen_h);
    }
    if m.header.repeat != 1 {
        let _ = writeln!(s, "repeat {}", m.header.repeat);
    }
    if m.header.speed_percent != 100 {
        let _ = writeln!(s, "speed {}", m.header.speed_percent);
    }
    for e in &m.events {
        if e.delay_us > 0 {
            let _ = writeln!(s, "wait {}", fmt_duration(e.delay_us));
        }
        match e.kind {
            EventType::MoveAbs => {
                let _ = writeln!(s, "move {} {}", e.x, e.y);
            }
            EventType::MoveRel => {
                let _ = writeln!(s, "moverel {} {}", e.x, e.y);
            }
            EventType::ButtonPress => {
                let _ = writeln!(s, "buttondown {}", keys::button_name(e.code));
            }
            EventType::ButtonRelease => {
                let _ = writeln!(s, "buttonup {}", keys::button_name(e.code));
            }
            EventType::KeyPress | EventType::KeyRelease => {
                let name = keys::key_name(e.code)
                    .map(str::to_string)
                    .unwrap_or_else(|| e.code.to_string());
                let verb = if e.kind == EventType::KeyPress {
                    "keydown"
                } else {
                    "keyup"
                };
                let _ = writeln!(s, "{verb} {name}");
            }
            EventType::Scroll => {
                let _ = writeln!(s, "scroll {} {}", e.x, e.y);
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{BTN_LEFT, Header};

    #[test]
    fn durations() {
        assert_eq!(parse_duration_us("50ms"), Some(50_000));
        assert_eq!(parse_duration_us("1.5s"), Some(1_500_000));
        assert_eq!(parse_duration_us("300us"), Some(300));
        assert_eq!(parse_duration_us("42"), Some(42));
        assert_eq!(parse_duration_us("x"), None);
    }

    #[test]
    fn parse_script() {
        let m = parse(
            "# hello\nscreen 1920x1080\nrepeat 3\nmove 10 20\nwait 50ms\nclick left\nwait 1s\nkey a\nscroll down 2\nmoverel 1 -1\n",
        )
        .unwrap();
        assert_eq!(
            m.header,
            Header {
                screen_w: 1920,
                screen_h: 1080,
                repeat: 3,
                speed_percent: 100
            }
        );
        assert_eq!(
            m.events,
            vec![
                Event::move_abs(10, 20, 0),
                Event::button(BTN_LEFT, true, 50_000),
                Event::button(BTN_LEFT, false, 0),
                Event::key(30, true, 1_000_000),
                Event::key(30, false, 0),
                Event::scroll(0, 240, 0),
                Event::move_rel(1, -1, 0),
            ]
        );
    }

    #[test]
    fn errors_have_line_numbers() {
        match parse("move 1 2\nbogus\n") {
            Err(Error::Syntax { line: 2, .. }) => {}
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            parse("keydown KEY_NOPE"),
            Err(Error::Syntax { line: 1, .. })
        ));
    }

    #[test]
    fn render_roundtrip() {
        let m = parse("screen 800x600\nmove 1 2\nwait 3ms\nbuttondown right\nwait 7us\nbuttonup right\nkeydown KEY_ESC\nkeyup KEY_ESC\nscroll -120 0\n").unwrap();
        let text = render(&m);
        assert_eq!(parse(&text).unwrap(), m);
        assert!(text.contains("wait 3ms"));
        assert!(text.contains("keydown KEY_ESC"));
        assert!(text.contains("buttondown right"));
    }
}
