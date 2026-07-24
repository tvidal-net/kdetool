use std::fmt::{self, Display};

/// Error produced while reading HOCON, either during parsing (with a source
/// location) or from serde during deserialization (location-less).
///
/// The module depends only on `serde`, so this type deliberately avoids any
/// `crate::` types: it can move to a standalone crate unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
    // 1-based; `0` means "no location" (e.g. errors raised by serde).
    line: usize,
    column: usize,
}

impl Error {
    /// A parse error anchored at a source location.
    pub(crate) fn at(line: usize, column: usize, message: impl Into<String>) -> Self {
        Error {
            message: message.into(),
            line,
            column,
        }
    }

    /// A location-less error (used for the serde `Error` trait and lookups).
    pub(crate) fn msg(message: impl Into<String>) -> Self {
        Error {
            message: message.into(),
            line: 0,
            column: 0,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(
                f,
                "{} (line {}, column {})",
                self.message, self.line, self.column
            )
        } else {
            f.write_str(&self.message)
        }
    }
}

impl std::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::msg(msg.to_string())
    }
}
