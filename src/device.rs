// SPDX-License-Identifier: Apache-2.0
//! USB HID transport for the YKUSH3 board.
//!
//! Talks to the board through hidapi, which reaches the device by way of IOKit.

use std::sync::OnceLock;

use hidapi::HidApi;

use crate::error::{Error, Result};

/// Microchip vendor id used by all Yepkit boards.
pub const VENDOR_ID: u16 = 0x04D8;
/// Product id of the YKUSH3 board.
pub const PRODUCT_ID: u16 = 0xF11B;

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
        let serial = dev.serial_number().unwrap_or("<unknown>").to_owned();
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

    /// Needs the HID stack of the operating system, but no board.
    /// Run with `cargo test -- --ignored --test-threads=1`.
    #[test]
    #[ignore = "needs the HID stack of the operating system"]
    fn enumeration_lists_the_attached_boards() {
        let serials = list().expect("enumeration should work");

        assert!(
            serials.iter().all(|s| !s.is_empty()),
            "a serial number should never be empty: {serials:?}"
        );
    }
}
