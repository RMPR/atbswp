//! Standalone executables: player bytes + payload + 16-byte footer.

use crate::format::{FOOTER_LEN, MAGIC, Macro};
use crate::{Error, Result};
use std::io::Write;

/// True when `bytes` end with our footer.
pub fn has_payload(bytes: &[u8]) -> bool {
    bytes.len() >= FOOTER_LEN && &bytes[bytes.len() - 8..] == MAGIC
}

/// Split an executable into (player, payload).
pub fn split(bytes: &[u8]) -> Result<(&[u8], &[u8])> {
    if !has_payload(bytes) {
        return Err(Error::Invalid("no macro footer found".into()));
    }
    let len_off = bytes.len() - FOOTER_LEN;
    let payload_len = u64::from_le_bytes(bytes[len_off..len_off + 8].try_into().unwrap()) as usize;
    if payload_len > len_off {
        return Err(Error::Invalid("footer length exceeds file".into()));
    }
    let start = len_off - payload_len;
    Ok((&bytes[..start], &bytes[start..len_off]))
}

/// Extract the macro embedded in an executable.
pub fn extract(bytes: &[u8]) -> Result<Macro> {
    let (_, payload) = split(bytes)?;
    Macro::decode(payload)
}

/// Return the bare player from an executable (strips a payload if present).
pub fn player_of(bytes: &[u8]) -> &[u8] {
    split(bytes).map(|(p, _)| p).unwrap_or(bytes)
}

/// Write `player` followed by the macro payload and footer to `out`.
pub fn write<W: Write>(player: &[u8], m: &Macro, mut out: W) -> Result<()> {
    if player.is_empty() {
        return Err(Error::Invalid("player binary is empty".into()));
    }
    let payload = m.encode();
    out.write_all(player_of(player))?;
    out.write_all(&payload)?;
    out.write_all(&(payload.len() as u64).to_le_bytes())?;
    out.write_all(MAGIC)?;
    Ok(())
}

/// Convenience: build the whole executable in memory.
pub fn build(player: &[u8], m: &Macro) -> Result<Vec<u8>> {
    let mut v = Vec::with_capacity(player.len() + m.events.len() * 16 + 64);
    write(player, m, &mut v)?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{Event, Header};

    #[test]
    fn embed_and_extract() {
        let player = b"MZqFpD='\nfake player bytes";
        let m = Macro {
            header: Header {
                screen_w: 800,
                screen_h: 600,
                ..Default::default()
            },
            events: vec![Event::key(1, true, 0), Event::key(1, false, 10)],
        };
        let exe = build(player, &m).unwrap();
        assert!(exe.starts_with(player));
        assert!(has_payload(&exe));
        assert_eq!(extract(&exe).unwrap(), m);
        assert_eq!(player_of(&exe), player);
        // Re-embedding into an already-embedded exe replaces the payload.
        let m2 = Macro {
            events: vec![],
            ..m.clone()
        };
        let exe2 = build(&exe, &m2).unwrap();
        assert_eq!(extract(&exe2).unwrap(), m2);
        assert_eq!(player_of(&exe2), player);
        assert_eq!(Macro::load(&exe).unwrap(), m);
    }

    #[test]
    fn empty_player_rejected() {
        assert!(build(&[], &Macro::default()).is_err());
    }
}
