// SPDX-License-Identifier: Apache-2.0
//! USB HID transport for the YKUSH3 board.
//!
//! Talks to the board through async-hid, which reaches the device by way of
//! IOKit. That library is asynchronous because that is how the operating
//! system delivers HID reports — it hands out callbacks on a run loop rather
//! than a blocking read. This program is not asynchronous and has no reason to
//! be, so the awaits are resolved here and everything above the [`Transport`]
//! trait stays as synchronous as it was.

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use std::time::Duration;

use async_hid::{AsyncHidRead, AsyncHidWrite, Device, DeviceReaderWriter, HidBackend};
use futures_lite::{future, StreamExt};

use crate::error::{Error, Result};
use crate::sanitize::sanitize;

/// Microchip vendor id used by all Yepkit boards.
pub const VENDOR_ID: u16 = 0x04D8;
/// Product id of the YKUSH3 board.
pub const PRODUCT_ID: u16 = 0xF11B;

/// Every YKUSH3 report is a fixed size block.
pub const REPORT_SIZE: usize = 64;

const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait for a late answer when emptying the queue after a read
/// timed out. Long enough to catch one already on its way, short enough not to
/// be felt on the next command.
const DISCARD_WAIT: Duration = Duration::from_millis(50);

/// Upper bound on the reports dropped in one go, so a board that streams cannot
/// hold the program in the drain.
const DISCARD_LIMIT: usize = 16;

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

/// An opened YKUSH3 board.
pub struct Board {
    /// Every exchange needs the handle mutably while [`Transport`] hands out a
    /// shared reference. A cell bridges the two, which costs nothing here: a
    /// board is driven from one thread at a time, and the operating system
    /// hands out a HID device exclusively anyway.
    dev: RefCell<DeviceReaderWriter>,

    /// Set when a read timed out. The answer may still arrive afterwards and
    /// would then be queued, so the next exchange empties the queue first.
    stale: Cell<bool>,
}

impl Board {
    /// Opens the board with the given serial number, or the first board in the
    /// enumeration order when no serial number is provided.
    pub fn open(serial: Option<&str>) -> Result<Self> {
        future::block_on(async {
            // Probing the device list first turns "nothing attached" into a
            // clear message instead of a generic open failure.
            let attached = attached_boards().await?;
            let chosen = match serial {
                Some(wanted) => attached.into_iter().find(|(known, _)| known == wanted),
                None => attached.into_iter().next(),
            };

            let Some((_, device)) = chosen else {
                return Err(Error::NotFound {
                    serial: serial.map(str::to_owned),
                });
            };

            Ok(Board {
                dev: RefCell::new(device.open().await?),
                stale: Cell::new(false),
            })
        })
    }

    /// Drops reports the board sent after a read had already given up on them.
    ///
    /// Bounded twice: by a short wait, so a board with nothing left to say
    /// costs almost nothing, and by a count, so one that streams cannot keep
    /// the program here. Errors are swallowed - the exchange that follows
    /// raises them properly.
    fn discard_queued(&self) {
        let mut dev = self.dev.borrow_mut();
        let mut scratch: Report = [0; REPORT_SIZE];

        for _ in 0..DISCARD_LIMIT {
            let got = future::block_on(future::or(
                async { Some(dev.read_input_report(&mut scratch).await) },
                async {
                    async_io::Timer::after(DISCARD_WAIT).await;
                    None
                },
            ));

            if !matches!(got, Some(Ok(_))) {
                break;
            }
        }
    }
}

impl Transport for Board {
    fn transfer(&self, out: &Report) -> Result<Report> {
        // An answer to a read that timed out may arrive afterwards and wait in
        // the queue of the transport. Taken for the answer to this command it
        // would pass unnoticed: the switching and configuration commands carry
        // nothing beyond the status byte to check it against. Emptying the
        // queue first is what keeps one timeout from shifting every later
        // answer by one.
        if self.stale.replace(false) {
            self.discard_queued();
        }

        self.send(out)?;

        let mut resp: Report = [0; REPORT_SIZE];
        let mut dev = self.dev.borrow_mut();

        // The library has no timed read, so the read races a timer: a board
        // that says nothing must not hang the program. Dropping the losing
        // read leaves the handle usable — the next command goes through as if
        // nothing had happened.
        let read = future::block_on(future::or(
            async { Some(dev.read_input_report(&mut resp).await) },
            async {
                async_io::Timer::after(READ_TIMEOUT).await;
                None
            },
        ));
        drop(dev);

        match read {
            None => {
                self.stale.set(true);
                Err(Error::NoResponse)
            }
            Some(Err(e)) => Err(e.into()),
            // The board always answers with a full report. Anything shorter
            // would leave the zero padding of the buffer to be read as answer
            // bytes, so it is rejected rather than interpreted.
            Some(Ok(read)) if read != REPORT_SIZE => Err(Error::Device(format!(
                "Truncated answer from the board: {read} of {REPORT_SIZE} bytes"
            ))),
            Some(Ok(_)) => Ok(resp),
        }
    }

    fn send(&self, out: &Report) -> Result<()> {
        // The first byte is the report id, which the library strips when it is
        // zero. The board uses unnumbered reports, so a leading zero is what it
        // wants: passing the bare report instead would send it under whatever
        // its first byte happens to be.
        //
        // There is no timeout here. The library offers none for writing, so a
        // device that stalls its endpoint blocks for as long as the operating
        // system lets it.
        let mut buf = [0u8; REPORT_SIZE + 1];
        buf[1..].copy_from_slice(out);

        let mut dev = self.dev.borrow_mut();
        future::block_on(dev.write_output_report(&buf))?;
        Ok(())
    }
}

/// Serial numbers of all attached YKUSH3 boards, in enumeration order.
pub fn list() -> Result<Vec<String>> {
    future::block_on(async {
        let attached = attached_boards().await?;
        Ok(attached.into_iter().map(|(serial, _)| serial).collect())
    })
}

/// Every attached board, as its sanitised serial number and the handle it was
/// found under, in enumeration order.
async fn attached_boards() -> Result<Vec<(String, Device)>> {
    let mut found: Vec<(String, Device)> = Vec::new();

    // One backend for the process. It is a handle rather than a resource, but
    // making it once keeps the enumeration from building it again per call.
    static BACKEND: OnceLock<HidBackend> = OnceLock::new();
    let backend = BACKEND.get_or_init(HidBackend::default);

    let mut devices = backend.enumerate().await?;
    while let Some(dev) = devices.next().await {
        if dev.vendor_id != VENDOR_ID || dev.product_id != PRODUCT_ID {
            continue;
        }
        let serial = sanitize(dev.serial_number.as_deref().unwrap_or("<unknown>"));
        // A board can expose several HID interfaces and would then show up
        // more than once in the device list.
        if !found.iter().any(|(known, _)| *known == serial) {
            found.push((serial, dev));
        }
    }

    Ok(found)
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
