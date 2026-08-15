// SPDX-License-Identifier: Apache-2.0
//! Error type shared by the command line, transport and protocol layers.

use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// The command line could not be interpreted.
    Usage(String),
    /// Writing the output failed.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(msg) => write!(f, "{msg}"),
            Error::Io(e) => write!(f, "Output error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_error() -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "boom")
    }

    #[test]
    fn every_error_says_what_went_wrong() {
        let cases: Vec<(Error, &str)> = vec![
            (Error::Usage("bad option".into()), "bad option"),
            (Error::Io(io_error()), "Output error"),
        ];

        for (error, expected) in cases {
            let message = error.to_string();
            assert!(message.contains(expected), "{message:?} lacks {expected:?}");
        }
    }

    #[test]
    fn output_failures_convert_into_an_error() {
        let io: Error = io_error().into();

        assert!(matches!(io, Error::Io(_)));
    }
}
