// SPDX-License-Identifier: Apache-2.0
//! USB HID transport for the YKUSH3 board.
//!
//! Talks to the board through hidapi, which reaches the device by way of IOKit.

use std::sync::OnceLock;

use hidapi::{HidApi, HidDevice};

use crate::error::{Error, Result};
use crate::sanitize::sanitize;

/// Microchip vendor id used by all Yepkit boards.
pub const VENDOR_ID: u16 = 0x04D8;
/// Product id of the YKUSH3 board.
pub const PRODUCT_ID: u16 = 0xF11B;

/// Every YKUSH3 report is a fixed size block.
pub const REPORT_SIZE: usize = 64;

const READ_TIMEOUT_MS: i32 = 5000;

pub type Report = [u8; REPORT_SIZE];

/// Report exchange with a board.
///
/// The protocol layer is written against this trait so it can be driven by a
/// test double instead of real hardware.
pub trait Transport {
    /// Sends a report and returns the report the board answers with.
    fn transfer(&self, out: &Report) -> Result<Report>;

    /// Sends a report without waiting for an answer. Used for the commands that
    /// make the board reboot, which therefore never answer.
    fn send(&self, out: &Report) -> Result<()>;
}

/// Pads a command payload to a full report.
///
/// Crate-internal on purpose: the debug assertion below is the only length
/// guard, which is fine for the fixed payloads of this crate but would be an
/// implicit panic in a public interface.
pub(crate) fn report(payload: &[u8]) -> Report {
    debug_assert!(payload.len() <= REPORT_SIZE);

    let mut buf: Report = [0; REPORT_SIZE];
    buf[..payload.len()].copy_from_slice(payload);
    buf
}

/// The process-wide HID stack.
///
/// Creating a `HidApi` runs `hid_init()` and dropping it runs `hid_exit()`,
/// both of which mutate global state in the C library. Initialising once and
/// never tearing it down keeps that state stable — on macOS a second
/// initialisation from a different thread aborts the process.
fn api() -> Result<&'static HidApi> {
    static API: OnceLock<std::result::Result<HidApi, String>> = OnceLock::new();

    match API.get_or_init(|| HidApi::new().map_err(|e| e.to_string())) {
        Ok(api) => Ok(api),
        Err(message) => Err(Error::HidInit(message.clone())),
    }
}

/// An opened YKUSH3 board.
pub struct Board {
    dev: HidDevice,
}

impl Board {
    /// Opens the board with the given serial number, or the first board in the
    /// enumeration order when no serial number is provided.
    pub fn open(serial: Option<&str>) -> Result<Self> {
        let api = api()?;

        // Probing the device list first turns "nothing attached" into a clear
        // message instead of a generic hidapi open failure.
        let attached = boards(api);
        let present = match serial {
            Some(s) => attached.iter().any(|b| b == s),
            None => !attached.is_empty(),
        };
        if !present {
            return Err(Error::NotFound {
                serial: serial.map(str::to_owned),
            });
        }

        let dev = match serial {
            Some(s) => api.open_serial(VENDOR_ID, PRODUCT_ID, s)?,
            None => api.open(VENDOR_ID, PRODUCT_ID)?,
        };

        Ok(Board { dev })
    }
}

impl Transport for Board {
    fn transfer(&self, out: &Report) -> Result<Report> {
        self.send(out)?;

        let mut resp: Report = [0; REPORT_SIZE];
        let read = self.dev.read_timeout(&mut resp, READ_TIMEOUT_MS)?;
        if read == 0 {
            return Err(Error::NoResponse);
        }
        // The board always answers with a full report. Anything shorter would
        // leave the zero padding of the buffer to be read as answer bytes, so
        // it is rejected rather than interpreted.
        if read != REPORT_SIZE {
            return Err(Error::Device(format!(
                "Truncated answer from the board: {read} of {REPORT_SIZE} bytes"
            )));
        }

        Ok(resp)
    }

    fn send(&self, out: &Report) -> Result<()> {
        // hidapi expects the report id in the first byte. The board uses
        // unnumbered reports, so a leading zero is prepended.
        //
        // Unlike the read above, the write has no timeout: hidapi offers no
        // timed variant, so a device that stalls its endpoint blocks here for
        // as long as the operating system lets it.
        let mut buf = [0u8; REPORT_SIZE + 1];
        buf[1..].copy_from_slice(out);
        let written = self.dev.write(&buf)?;
        if written != buf.len() {
            return Err(Error::Device(format!(
                "Truncated write to the board: {written} of {} bytes",
                buf.len()
            )));
        }
        Ok(())
    }
}

/// Serial numbers of all attached YKUSH3 boards, in enumeration order.
pub fn list() -> Result<Vec<String>> {
    Ok(boards(api()?))
}

fn boards(api: &HidApi) -> Vec<String> {
    let mut serials: Vec<String> = Vec::new();
    for dev in api.device_list() {
        if dev.vendor_id() != VENDOR_ID || dev.product_id() != PRODUCT_ID {
            continue;
        }
        let serial = sanitize(dev.serial_number().unwrap_or("<unknown>"));
        // A board can expose several HID interfaces and would then show up
        // more than once in the device list.
        if !serials.contains(&serial) {
            serials.push(serial);
        }
    }
    serials
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_is_padded_to_a_full_report() {
        let r = report(&[0x52, 0x01, 0x20, 0x02, 0xaa, 0xbb]);

        assert_eq!(r.len(), REPORT_SIZE);
        assert_eq!(&r[..6], &[0x52, 0x01, 0x20, 0x02, 0xaa, 0xbb]);
        assert!(r[6..].iter().all(|&b| b == 0));
    }

    /// Everything below needs a YKUSH3 attached and is skipped by default.
    ///
    /// Run with `cargo test -- --ignored --test-threads=1`. The single thread is
    /// not optional: the operating system hands out a HID device exclusively,
    /// and opening or closing it from several threads at once takes the whole
    /// process down.
    mod hardware {
        use super::*;

        #[test]
        #[ignore = "needs the HID stack of the operating system"]
        fn enumeration_lists_the_attached_boards() {
            let serials = list().expect("enumeration should work");

            assert!(
                serials.iter().all(|s| !s.is_empty()),
                "a serial number should never be empty: {serials:?}"
            );
        }

        fn attached() -> Vec<String> {
            let serials = list().expect("enumeration should work");
            assert!(!serials.is_empty(), "no YKUSH3 attached");
            serials
        }

        #[test]
        #[ignore = "needs a YKUSH3 attached"]
        fn a_board_is_found_and_answers_a_report() {
            let serials = attached();

            let board = Board::open(None).expect("the first board should open");
            let answer = board
                .transfer(&report(&[0x61, 0x02]))
                .expect("the board should answer the firmware version");

            assert_eq!(answer[0], 0x01, "status byte");
            assert_eq!(answer[1], 0x61, "echoed opcode");
            assert_eq!(serials.len(), list().unwrap().len(), "stable enumeration");
        }

        #[test]
        #[ignore = "needs a YKUSH3 attached"]
        fn a_board_can_be_addressed_by_serial_number() {
            let serials = attached();

            let board = Board::open(Some(&serials[0])).expect("open by serial number");
            let answer = board.transfer(&report(&[0x61, 0x01])).expect("answer");

            assert_eq!(answer[1], 0x61);
        }

        #[test]
        #[ignore = "needs a YKUSH3 attached"]
        fn an_unknown_serial_number_is_reported_as_not_found() {
            attached();

            let result = Board::open(Some("no-such-board"));

            assert!(matches!(result, Err(Error::NotFound { serial: Some(_) })));
        }
    }
}
