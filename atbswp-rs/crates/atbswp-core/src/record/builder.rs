//! Turns timestamped raw input into macro events: coalesces motion, drops
//! the stop key, releases whatever is still held at the end.  Shared by all
//! platform recorders and unit tested on its own.

use super::Options;
use atbswp_macro::{Event, Header, Macro};

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
    last_abs: Option<(i32, i32)>,
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
            last_abs: None,
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
        if let Some((_, last, x, y)) = self.pending_abs.take()
            && self.last_abs != Some((x, y))
        {
            let d = self.delay_for(last);
            self.events.push(Event::move_abs(x, y, d));
            self.last_abs = Some((x, y));
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
    fn repeated_absolute_position_is_not_duplicated() {
        let mut b = Builder::new(&opts());
        b.move_abs(0, 5, 5);
        b.move_abs(20_000, 5, 5);
        b.move_abs(40_000, 5, 5);
        b.move_abs(60_000, 6, 6);
        let m = b.finish(Header::default());
        assert_eq!(
            m.events
                .iter()
                .filter(|e| e.kind == EventType::MoveAbs)
                .count(),
            2
        );
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
