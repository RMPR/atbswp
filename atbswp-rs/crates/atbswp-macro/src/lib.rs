//! Macro payload format shared by the atbswp recorder and the standalone
//! player (`player/src/macro_format.h` is the C twin of [`format`]).
//!
//! A macro is a header plus a flat list of fixed-size events.  It can be
//! stored three ways, all handled by [`Macro::load`]:
//!
//! * raw binary payload (`.atbswp`),
//! * a human-editable text script (see [`text`]),
//! * appended to a copy of the portable player, which yields a standalone
//!   executable (see [`exe`]).
//!
//! The crate has no dependencies so the CLI stays small and quick to build.

pub mod exe;
pub mod format;
pub mod keys;
pub mod text;

pub use format::{Event, EventType, Header, Macro};

use std::fmt;

/// Errors produced while parsing or writing macros.
#[derive(Debug)]
pub enum Error {
    /// The data is not a valid payload/executable/text macro.
    Invalid(String),
    /// Unsupported format version.
    Version(u16),
    /// Text script syntax error with 1-based line number.
    Syntax {
        line: usize,
        msg: String,
    },
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Invalid(m) => write!(f, "invalid macro: {m}"),
            Error::Version(v) => write!(f, "unsupported macro format version {v}"),
            Error::Syntax { line, msg } => write!(f, "line {line}: {msg}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
