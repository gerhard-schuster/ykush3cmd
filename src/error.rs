// SPDX-License-Identifier: Apache-2.0
//! Error type shared by the command line, transport and protocol layers.

use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// The command line could not be interpreted.
    Usage(String),
    /// The HID stack refused the operation.
    Hid(hidapi::HidError),
    /// The HID stack could not be initialised at all.
    HidInit(String),
    /// Writing the output failed.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(msg) => write!(f, "{msg}"),
            Error::Hid(e) => write!(f, "USB HID error: {e}"),
            Error::HidInit(msg) => write!(f, "Cannot use the USB HID stack: {msg}"),
            Error::Io(e) => write!(f, "Output error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<hidapi::HidError> for Error {
    fn from(e: hidapi::HidError) -> Self {
        Error::Hid(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hid_error() -> hidapi::HidError {
        hidapi::HidError::HidApiError {
            message: "boom".into(),
        }
    }

    fn io_error() -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "boom")
    }

    #[test]
    fn every_error_says_what_went_wrong() {
        let cases: Vec<(Error, &str)> = vec![
            (Error::Usage("bad option".into()), "bad option"),
            (Error::Hid(hid_error()), "USB HID error"),
            (
                Error::HidInit("no permission".into()),
                "Cannot use the USB HID stack",
            ),
            (Error::Io(io_error()), "Output error"),
        ];

        for (error, expected) in cases {
            let message = error.to_string();
            assert!(message.contains(expected), "{message:?} lacks {expected:?}");
        }
    }

    #[test]
    fn transport_and_output_failures_convert_into_an_error() {
        let hid: Error = hid_error().into();
        let io: Error = io_error().into();

        assert!(matches!(hid, Error::Hid(_)));
        assert!(matches!(io, Error::Io(_)));
    }
}
