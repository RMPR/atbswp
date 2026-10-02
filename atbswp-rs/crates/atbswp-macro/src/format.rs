//! Binary layout.  Keep in sync with `player/src/macro_format.h`.

use crate::{Error, Result};

pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_LEN: usize = 32;
pub const EVENT_LEN: usize = 16;

/// evdev button codes.
pub const BTN_LEFT: u16 = 0x110;
pub const BTN_RIGHT: u16 = 0x111;
pub const BTN_MIDDLE: u16 = 0x112;
pub const BTN_SIDE: u16 = 0x113;
pub const BTN_EXTRA: u16 = 0x114;

/// Scroll unit: one wheel notch.
pub const SCROLL_NOTCH: i32 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum EventType {
    /// `x`,`y`: absolute pointer position in recorded screen pixels.
    MoveAbs = 1,
    /// `x`,`y`: relative pointer delta.
    MoveRel = 2,
    /// `code`: evdev `BTN_*`.
    ButtonPress = 3,
    ButtonRelease = 4,
    /// `code`: evdev `KEY_*`.
    KeyPress = 5,
    KeyRelease = 6,
    /// `x`,`y`: 1/120 of a notch; +y = down, +x = right.
    Scroll = 7,
}

impl EventType {
    pub fn from_u16(v: u16) -> Option<Self> {
        Some(match v {
            1 => Self::MoveAbs,
            2 => Self::MoveRel,
            3 => Self::ButtonPress,
            4 => Self::ButtonRelease,
            5 => Self::KeyPress,
            6 => Self::KeyRelease,
            7 => Self::Scroll,
            _ => return None,
        })
    }
}

/// One recorded input event. `delay_us` is the pause *before* the event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventType,
    pub code: u16,
    pub x: i32,
    pub y: i32,
    pub delay_us: u32,
}

impl Event {
    pub fn new(kind: EventType, delay_us: u32) -> Self {
        Event {
            kind,
            code: 0,
            x: 0,
            y: 0,
            delay_us,
        }
    }
    pub fn move_abs(x: i32, y: i32, delay_us: u32) -> Self {
        Event {
            x,
            y,
            ..Self::new(EventType::MoveAbs, delay_us)
        }
    }
    pub fn move_rel(x: i32, y: i32, delay_us: u32) -> Self {
        Event {
            x,
            y,
            ..Self::new(EventType::MoveRel, delay_us)
        }
    }
    pub fn button(code: u16, pressed: bool, delay_us: u32) -> Self {
        let kind = if pressed {
            EventType::ButtonPress
        } else {
            EventType::ButtonRelease
        };
        Event {
            code,
            ..Self::new(kind, delay_us)
        }
    }
    pub fn key(code: u16, pressed: bool, delay_us: u32) -> Self {
        let kind = if pressed {
            EventType::KeyPress
        } else {
            EventType::KeyRelease
        };
        Event {
            code,
            ..Self::new(kind, delay_us)
        }
    }
    pub fn scroll(x: i32, y: i32, delay_us: u32) -> Self {
        Event {
            x,
            y,
            ..Self::new(EventType::Scroll, delay_us)
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.kind as u16).to_le_bytes());
        out.extend_from_slice(&self.code.to_le_bytes());
        out.extend_from_slice(&self.x.to_le_bytes());
        out.extend_from_slice(&self.y.to_le_bytes());
        out.extend_from_slice(&self.delay_us.to_le_bytes());
    }

    fn read(b: &[u8]) -> Result<Self> {
        let kind_raw = u16::from_le_bytes([b[0], b[1]]);
        let kind = EventType::from_u16(kind_raw)
            .ok_or_else(|| Error::Invalid(format!("unknown event type {kind_raw}")))?;
        Ok(Event {
            kind,
            code: u16::from_le_bytes([b[2], b[3]]),
            x: i32::from_le_bytes(b[4..8].try_into().unwrap()),
            y: i32::from_le_bytes(b[8..12].try_into().unwrap()),
            delay_us: u32::from_le_bytes(b[12..16].try_into().unwrap()),
        })
    }
}

/// Playback metadata stored ahead of the events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Screen size at recording time; 0 disables scaling on playback.
    pub screen_w: u32,
    pub screen_h: u32,
    /// Default repeat count, 0 = forever.
    pub repeat: u32,
    /// Default speed in percent, 100 = real time.
    pub speed_percent: u32,
}

impl Default for Header {
    fn default() -> Self {
        Header {
            screen_w: 0,
            screen_h: 0,
            repeat: 1,
            speed_percent: 100,
        }
    }
}

/// A complete macro.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Macro {
    pub header: Header,
    pub events: Vec<Event>,
}

impl Macro {
    /// Total duration at 100% speed.
    pub fn duration_us(&self) -> u64 {
        self.events.iter().map(|e| e.delay_us as u64).sum()
    }

    /// Serialise header + events (the "payload").
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.events.len() * EVENT_LEN);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&(self.events.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.header.screen_w.to_le_bytes());
        out.extend_from_slice(&self.header.screen_h.to_le_bytes());
        out.extend_from_slice(&self.header.repeat.to_le_bytes());
        out.extend_from_slice(&self.header.speed_percent.to_le_bytes());
        out.extend_from_slice(&[0u8; 8]); // reserved
        debug_assert_eq!(out.len(), HEADER_LEN);
        for e in &self.events {
            e.write(&mut out);
        }
        out
    }

    /// Parse a raw payload produced by [`Macro::encode`].
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() < HEADER_LEN {
            return Err(Error::Invalid("payload shorter than header".into()));
        }
        let version = u16::from_le_bytes([b[0], b[1]]);
        if version != FORMAT_VERSION {
            return Err(Error::Version(version));
        }
        let count = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
        let header = Header {
            screen_w: u32::from_le_bytes(b[8..12].try_into().unwrap()),
            screen_h: u32::from_le_bytes(b[12..16].try_into().unwrap()),
            repeat: u32::from_le_bytes(b[16..20].try_into().unwrap()),
            speed_percent: u32::from_le_bytes(b[20..24].try_into().unwrap()),
        };
        let body = &b[HEADER_LEN..];
        if body.len() != count * EVENT_LEN {
            return Err(Error::Invalid(format!(
                "expected {} event bytes, found {}",
                count * EVENT_LEN,
                body.len()
            )));
        }
        let events = body
            .as_chunks::<EVENT_LEN>()
            .0
            .iter()
            .map(|c| Event::read(c))
            .collect::<Result<Vec<_>>>()?;
        Ok(Macro { header, events })
    }

    /// Detect the container format and load from any supported representation.
    pub fn load(bytes: &[u8]) -> Result<Self> {
        if exe::has_payload(bytes) {
            return exe::extract(bytes);
        }
        if bytes.len() >= HEADER_LEN
            && bytes[0..2] == FORMAT_VERSION.to_le_bytes()
            && let Ok(m) = Self::decode(bytes)
        {
            return Ok(m);
        }
        let s = std::str::from_utf8(bytes)
            .map_err(|_| Error::Invalid("not a payload, executable, or UTF-8 script".into()))?;
        crate::text::parse(s)
    }
}

use crate::exe;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Macro {
        Macro {
            header: Header {
                screen_w: 1920,
                screen_h: 1080,
                repeat: 2,
                speed_percent: 150,
            },
            events: vec![
                Event::move_abs(10, 20, 0),
                Event::button(BTN_LEFT, true, 1000),
                Event::button(BTN_LEFT, false, 2000),
                Event::key(30, true, 5),
                Event::key(30, false, 6),
                Event::scroll(0, -240, 7),
                Event::move_rel(-3, 4, 8),
            ],
        }
    }

    #[test]
    fn roundtrip() {
        let m = sample();
        let bytes = m.encode();
        assert_eq!(bytes.len(), HEADER_LEN + 7 * EVENT_LEN);
        assert_eq!(Macro::decode(&bytes).unwrap(), m);
        assert_eq!(Macro::load(&bytes).unwrap(), m);
    }

    #[test]
    fn layout_matches_c_header() {
        let m = Macro {
            events: vec![Event::scroll(1, -2, 3)],
            ..Default::default()
        };
        let b = m.encode();
        assert_eq!(&b[0..2], &[1, 0]); // version
        assert_eq!(&b[4..8], &[1, 0, 0, 0]); // count
        assert_eq!(&b[16..20], &[1, 0, 0, 0]); // repeat
        assert_eq!(&b[20..24], &[100, 0, 0, 0]); // speed
        let e = &b[HEADER_LEN..];
        assert_eq!(&e[0..2], &[7, 0]);
        assert_eq!(&e[4..8], &1i32.to_le_bytes());
        assert_eq!(&e[8..12], &(-2i32).to_le_bytes());
        assert_eq!(&e[12..16], &3u32.to_le_bytes());
    }

    #[test]
    fn rejects_garbage() {
        assert!(Macro::decode(&[0u8; 10]).is_err());
        let mut b = sample().encode();
        b.truncate(b.len() - 1);
        assert!(Macro::decode(&b).is_err());
        b[0] = 9;
        assert!(matches!(Macro::decode(&b), Err(Error::Version(9))));
    }
}
