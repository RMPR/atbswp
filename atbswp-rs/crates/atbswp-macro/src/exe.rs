//! Standalone executables: the player with the macro stored as the zip
//! entry `macro.bin` (see `player/src/macro_format.h`).

use crate::format::Macro;
use crate::zip;
use crate::{Error, Result};
use std::io::Write;

pub const MACRO_ENTRY: &str = "macro.bin";

/// True when `bytes` carry a macro.
pub fn has_payload(bytes: &[u8]) -> bool {
    matches!(zip::read(bytes, MACRO_ENTRY), Ok(Some(_)))
}

/// Extract the macro embedded in an executable.
pub fn extract(bytes: &[u8]) -> Result<Macro> {
    match zip::read(bytes, MACRO_ENTRY)? {
        Some(p) => Macro::decode(p),
        None => Err(Error::Invalid("no macro.bin entry found".into())),
    }
}

/// Bytes of the executable with any macro removed, so an exported macro can
/// serve as the player for another export.
pub fn player_of(bytes: &[u8]) -> Result<Vec<u8>> {
    if has_payload(bytes) {
        zip::remove_last(bytes, MACRO_ENTRY)
    } else {
        Ok(bytes.to_vec())
    }
}

/// Build the whole executable in memory.
pub fn build(player: &[u8], m: &Macro) -> Result<Vec<u8>> {
    if player.is_empty() {
        return Err(Error::Invalid("player binary is empty".into()));
    }
    zip::append_stored(player, MACRO_ENTRY, &m.encode())
}

/// Write `player` + macro to `out`.
pub fn write<W: Write>(player: &[u8], m: &Macro, mut out: W) -> Result<()> {
    out.write_all(&build(player, m)?)?;
    Ok(())
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
        assert_eq!(player_of(&exe).unwrap(), player.to_vec());
        // Re-exporting from an exported macro replaces the payload in place.
        let m2 = Macro {
            events: vec![],
            ..m.clone()
        };
        let exe2 = build(&exe, &m2).unwrap();
        assert_eq!(extract(&exe2).unwrap(), m2);
        assert_eq!(Macro::load(&exe).unwrap(), m);
    }

    #[test]
    fn keeps_other_entries() {
        let player = zip::append_stored(b"MZ player", "player-macos-x86_64", b"MACHO").unwrap();
        let m = Macro::default();
        let exe = build(&player, &m).unwrap();
        assert_eq!(
            zip::read(&exe, "player-macos-x86_64").unwrap(),
            Some(&b"MACHO"[..])
        );
        assert_eq!(extract(&exe).unwrap(), m);
        assert_eq!(player_of(&exe).unwrap(), player);
        assert!(!has_payload(&player));
    }

    #[test]
    fn empty_player_rejected() {
        assert!(build(&[], &Macro::default()).is_err());
    }
}
