// SPDX-License-Identifier: Apache-2.0
//! Error type shared by the command line, transport and protocol layers.

use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// The command line could not be interpreted.
    Usage(String),
    /// No board (or no board with the requested serial number) is attached.
    NotFound { serial: Option<String> },
    /// The HID stack refused the operation. Carries the message rather than
    /// the hidapi error type, which would otherwise become part of this
    /// library's public interface and tie it to the hidapi major version.
    Hid(String),
    /// The HID stack could not be initialised at all.
    HidInit(String),
    /// The board did not answer within the read timeout.
    NoResponse,
    /// The board answered, but reported a failure or an unexpected report.
    Device(String),
    /// Writing the output failed.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(msg) => write!(f, "{msg}"),
            Error::NotFound { serial: Some(s) } => {
                write!(f, "No YKUSH3 board with serial number {s} found")
            }
            Error::NotFound { serial: None } => write!(f, "No YKUSH3 board found"),
            Error::Hid(e) => write!(f, "USB HID error: {e}"),
            Error::HidInit(msg) => write!(f, "Cannot use the USB HID stack: {msg}"),
            Error::NoResponse => write!(f, "No response from the board"),
            Error::Device(msg) => write!(f, "{msg}"),
            Error::Io(e) => write!(f, "Output error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    /// The underlying error where one is kept. `Hid` deliberately carries
    /// only the message, see the variant.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<hidapi::HidError> for Error {
    fn from(e: hidapi::HidError) -> Self {
        Error::Hid(e.to_string())
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
            (
                Error::NotFound {
                    serial: Some("YK00001".into()),
                },
                "serial number YK00001",
            ),
            (Error::NotFound { serial: None }, "No YKUSH3 board found"),
            (Error::NoResponse, "No response from the board"),
            (
                Error::Device("the board said no".into()),
                "the board said no",
            ),
            (Error::Hid("boom".into()), "USB HID error"),
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

    #[test]
    fn an_output_error_exposes_its_source() {
        use std::error::Error as _;

        let io: Error = io_error().into();
        let device = Error::Device("the board said no".into());

        assert!(io.source().is_some(), "Io keeps the underlying error");
        assert!(device.source().is_none(), "Device carries only a message");
    }
}
