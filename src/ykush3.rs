// SPDX-License-Identifier: Apache-2.0
//! YKUSH3 protocol layer.
//!
//! Report layout, opcodes and answer handling follow the control interface the
//! vendor documents, so this speaks the same wire protocol as their own
//! application.

use std::fmt;

use crate::device::{report, Board, Report, Transport};
use crate::error::{Error, Result};

/// A switchable power port of the board.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Port {
    /// Downstream USB port 1 to 3.
    Downstream(u8),
    /// The external 5V output port.
    External,
    /// All downstream ports at once.
    All,
}

impl Port {
    /// Port number as used in the low nibble of the port opcodes.
    fn code(self) -> u8 {
        match self {
            Port::Downstream(n) => n,
            Port::External => 0x4,
            Port::All => 0xa,
        }
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Port::Downstream(n) => write!(f, "Port {n}"),
            Port::External => write!(f, "External 5V port"),
            Port::All => write!(f, "All ports"),
        }
    }
}

/// Power-on default of a port, as stored in the board configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PowerOnState {
    Off,
    On,
    /// Restore the state the port had when the board lost power.
    Persist,
}

impl PowerOnState {
    fn code(self) -> u8 {
        match self {
            PowerOnState::Off => 0,
            PowerOnState::On => 1,
            PowerOnState::Persist => 2,
        }
    }
}

/// Switching state reported by the board for a single port.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PortStatus {
    pub port: u8,
    pub on: bool,
}

impl fmt::Display for PortStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Port {}: {}",
            self.port,
            if self.on { "on" } else { "off" }
        )
    }
}

/// A three part firmware or bootloader version.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Command opcodes understood by the board.
mod op {
    pub const PORT_DOWN: u8 = 0x00;
    pub const PORT_UP: u8 = 0x10;
    pub const PORT_STATUS: u8 = 0x20;
    pub const IO_READ: u8 = 0x30;
    pub const IO_WRITE: u8 = 0x31;
    pub const IO_CONTROL: u8 = 0x32;
    pub const PORT_CONFIG: u8 = 0x41;
    pub const BOOTLOADER: u8 = 0x42;
    pub const RESET: u8 = 0x55;
    pub const VERSION: u8 = 0x61;

    /// First byte of a successful answer.
    pub const ACK: u8 = 0x01;
}

/// Firmware versions before 1.1.0 do not know the version command.
const LEGACY_FIRMWARE: Version = Version { major: 1, minor: 0, patch: 0 };
const LEGACY_BOOTLOADER: Version = Version { major: 0, minor: 10, patch: 0 };

/// A YKUSH3 board ready to take commands.
pub struct Ykush3<T: Transport = Board> {
    transport: T,
}

impl Ykush3<Board> {
    /// Opens a board over USB.
    pub fn open(serial: Option<&str>) -> Result<Self> {
        Ok(Ykush3::with_transport(Board::open(serial)?))
    }
}

impl<T: Transport> Ykush3<T> {
    pub fn with_transport(transport: T) -> Self {
        Ykush3 { transport }
    }

    /// Powers a port up.
    pub fn port_up(&self, port: Port) -> Result<()> {
        self.request(&[op::PORT_UP | port.code()])?;
        Ok(())
    }

    /// Powers a port down.
    pub fn port_down(&self, port: Port) -> Result<()> {
        self.request(&[op::PORT_DOWN | port.code()])?;
        Ok(())
    }

    /// Reads the switching state of a single port.
    pub fn port_status(&self, port: Port) -> Result<PortStatus> {
        if port == Port::All {
            return Err(Error::Usage(
                "The switching state can only be read for a single port".into(),
            ));
        }

        let resp = self.request(&[op::PORT_STATUS | port.code()])?;

        // The answer carries the port number in the low nibble and the
        // switching state in the high nibble.
        let state = resp[1];
        let number = state & 0x0f;
        if number == 0 || number > 4 {
            return Err(Error::Device(format!(
                "Unexpected port state answer: 0x{state:02x}"
            )));
        }

        Ok(PortStatus {
            port: number,
            on: (state >> 4) != 0,
        })
    }

    /// Reads the level of a GPIO pin.
    pub fn read_io(&self, gpio: u8) -> Result<u8> {
        let resp = self.request(&[op::IO_READ, gpio])?;
        Ok(resp[3])
    }

    /// Drives a GPIO pin high or low.
    pub fn write_io(&self, gpio: u8, high: bool) -> Result<()> {
        self.request(&[op::IO_WRITE, gpio, u8::from(high)])?;
        Ok(())
    }

    /// Enables or disables the GPIO control interface. Takes effect on the next
    /// reset or power-on of the board.
    pub fn gpio_control(&self, enable: bool) -> Result<()> {
        self.request(&[op::IO_CONTROL, u8::from(enable)])?;
        Ok(())
    }

    /// Configures the power-on default state of a port.
    pub fn config_port(&self, port: Port, state: PowerOnState) -> Result<()> {
        if port == Port::All {
            return Err(Error::Usage(
                "The power-on state can only be configured for a single port".into(),
            ));
        }

        self.request(&[op::PORT_CONFIG, port.code(), state.code()])?;
        Ok(())
    }

    /// Reboots the board. The board does not answer this command.
    pub fn reset(&self) -> Result<()> {
        self.notify(&[op::RESET])
    }

    /// Enters and stays in bootloader mode. The board does not answer this
    /// command.
    pub fn enter_bootloader(&self) -> Result<()> {
        self.notify(&[op::BOOTLOADER])
    }

    /// Firmware version of the board.
    pub fn firmware_version(&self) -> Result<Version> {
        self.version(0x02, LEGACY_FIRMWARE)
    }

    /// Bootloader version of the board.
    pub fn bootloader_version(&self) -> Result<Version> {
        self.version(0x01, LEGACY_BOOTLOADER)
    }

    fn version(&self, kind: u8, legacy: Version) -> Result<Version> {
        let resp = self.request(&[op::VERSION, kind])?;

        // Boards too old to know the version command leave the answer empty.
        if resp[0] != op::ACK && resp[0] != op::VERSION {
            return Ok(legacy);
        }

        Ok(Version {
            major: resp[2],
            minor: resp[3],
            patch: resp[4],
        })
    }

    /// Sends a command and returns the answer.
    fn request(&self, payload: &[u8]) -> Result<Report> {
        self.transport.transfer(&report(payload))
    }

    /// Sends a command that the board does not answer.
    fn notify(&self, payload: &[u8]) -> Result<()> {
        self.transport.send(&report(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeBoard;

    /// Runs `command` against a board answering with `answer` and returns the
    /// command result together with the report that was sent.
    fn exchange<R>(
        answer: &[u8],
        command: impl FnOnce(&Ykush3<FakeBoard>) -> Result<R>,
        sent_len: usize,
    ) -> (Result<R>, Vec<u8>) {
        let board = Ykush3::with_transport(FakeBoard::answering(answer));
        let result = command(&board);
        let sent = board.transport.sent_payload(sent_len);
        (result, sent)
    }

    #[test]
    fn port_up_sends_the_documented_opcodes() {
        for (port, opcode) in [
            (Port::Downstream(1), 0x11),
            (Port::Downstream(2), 0x12),
            (Port::Downstream(3), 0x13),
            (Port::External, 0x14),
            (Port::All, 0x1a),
        ] {
            let (result, sent) = exchange(&[0x01], |b| b.port_up(port), 1);

            assert!(result.is_ok());
            assert_eq!(sent, vec![opcode], "port_up({port:?})");
        }
    }

    #[test]
    fn port_down_sends_the_documented_opcodes() {
        for (port, opcode) in [
            (Port::Downstream(1), 0x01),
            (Port::Downstream(2), 0x02),
            (Port::Downstream(3), 0x03),
            (Port::External, 0x04),
            (Port::All, 0x0a),
        ] {
            let (result, sent) = exchange(&[0x01], |b| b.port_down(port), 1);

            assert!(result.is_ok());
            assert_eq!(sent, vec![opcode], "port_down({port:?})");
        }
    }

    #[test]
    fn port_status_asks_for_the_requested_port() {
        for (port, opcode) in [
            (Port::Downstream(1), 0x21),
            (Port::Downstream(3), 0x23),
            (Port::External, 0x24),
        ] {
            let (_, sent) = exchange(&[0x01, 0x11], |b| b.port_status(port), 1);

            assert_eq!(sent, vec![opcode], "port_status({port:?})");
        }
    }

    #[test]
    fn port_status_decodes_the_state_nibble() {
        for (answer, expected) in [
            (0x11, PortStatus { port: 1, on: true }),
            (0x02, PortStatus { port: 2, on: false }),
            (0x13, PortStatus { port: 3, on: true }),
            // The external port is decoded the same way as the others.
            (0x14, PortStatus { port: 4, on: true }),
            (0x04, PortStatus { port: 4, on: false }),
        ] {
            let (result, _) = exchange(
                &[0x01, answer],
                |b| b.port_status(Port::Downstream(1)),
                1,
            );

            assert_eq!(result.unwrap(), expected, "answer 0x{answer:02x}");
        }
    }

    #[test]
    fn port_status_rejects_an_unexpected_answer() {
        let (result, _) = exchange(&[0x01, 0x55], |b| b.port_status(Port::Downstream(1)), 1);

        assert!(matches!(result, Err(Error::Device(_))));
    }

    #[test]
    fn ports_have_a_readable_name() {
        assert_eq!(Port::Downstream(2).to_string(), "Port 2");
        assert_eq!(Port::External.to_string(), "External 5V port");
        assert_eq!(Port::All.to_string(), "All ports");
    }

    #[test]
    fn port_status_rejects_all_ports() {
        let board = Ykush3::with_transport(FakeBoard::mute());

        let result = board.port_status(Port::All);

        assert!(matches!(result, Err(Error::Usage(_))));
        assert_eq!(board.transport.sent_count(), 0, "nothing must be sent");
    }

    #[test]
    fn read_io_returns_the_pin_level_from_the_answer() {
        let (result, sent) = exchange(&[0x01, 0x30, 0x02, 0x01], |b| b.read_io(2), 2);

        assert_eq!(sent, vec![0x30, 0x02]);
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn write_io_encodes_the_level() {
        let (_, high) = exchange(&[0x01], |b| b.write_io(3, true), 3);
        let (_, low) = exchange(&[0x01], |b| b.write_io(3, false), 3);

        assert_eq!(high, vec![0x31, 0x03, 0x01]);
        assert_eq!(low, vec![0x31, 0x03, 0x00]);
    }

    #[test]
    fn gpio_control_toggles_the_interface() {
        let (_, enable) = exchange(&[0x01], |b| b.gpio_control(true), 2);
        let (_, disable) = exchange(&[0x01], |b| b.gpio_control(false), 2);

        assert_eq!(enable, vec![0x32, 0x01]);
        assert_eq!(disable, vec![0x32, 0x00]);
    }

    #[test]
    fn config_port_encodes_port_and_power_on_state() {
        for (port, state, expected) in [
            (Port::Downstream(1), PowerOnState::Off, [0x41, 0x01, 0x00]),
            (Port::Downstream(2), PowerOnState::On, [0x41, 0x02, 0x01]),
            (Port::Downstream(3), PowerOnState::Persist, [0x41, 0x03, 0x02]),
            (Port::External, PowerOnState::On, [0x41, 0x04, 0x01]),
        ] {
            let (result, sent) = exchange(&[0x01], |b| b.config_port(port, state), 3);

            assert!(result.is_ok());
            assert_eq!(sent, expected.to_vec(), "config_port({port:?}, {state:?})");
        }
    }

    #[test]
    fn config_port_rejects_all_ports() {
        let board = Ykush3::with_transport(FakeBoard::mute());

        let result = board.config_port(Port::All, PowerOnState::On);

        assert!(matches!(result, Err(Error::Usage(_))));
        assert_eq!(board.transport.sent_count(), 0);
    }

    #[test]
    fn reset_does_not_wait_for_an_answer() {
        // FakeBoard::mute() has no answer queued, so a read would fail the test.
        let board = Ykush3::with_transport(FakeBoard::mute());

        board.reset().unwrap();

        assert_eq!(board.transport.sent_payload(1), vec![0x55]);
    }

    #[test]
    fn entering_the_bootloader_does_not_wait_for_an_answer() {
        let board = Ykush3::with_transport(FakeBoard::mute());

        board.enter_bootloader().unwrap();

        assert_eq!(board.transport.sent_payload(1), vec![0x42]);
    }

    #[test]
    fn versions_are_requested_by_selector_and_decoded() {
        let (firmware, fw_sent) = exchange(&[0x01, 0x61, 1, 2, 3], |b| b.firmware_version(), 2);
        let (boot, boot_sent) = exchange(&[0x01, 0x61, 0, 11, 4], |b| b.bootloader_version(), 2);

        assert_eq!(fw_sent, vec![0x61, 0x02]);
        assert_eq!(boot_sent, vec![0x61, 0x01]);
        assert_eq!(firmware.unwrap().to_string(), "1.2.3");
        assert_eq!(boot.unwrap().to_string(), "0.11.4");
    }

    #[test]
    fn an_unanswered_version_falls_back_to_the_legacy_version() {
        // Old boards do not know the command and leave the answer empty.
        let (firmware, _) = exchange(&[0x00], |b| b.firmware_version(), 2);
        let (boot, _) = exchange(&[0x00], |b| b.bootloader_version(), 2);

        assert_eq!(firmware.unwrap(), LEGACY_FIRMWARE);
        assert_eq!(boot.unwrap(), LEGACY_BOOTLOADER);
    }

    #[test]
    fn a_transport_error_reaches_the_caller() {
        let board = Ykush3::with_transport(FakeBoard::failing(Error::NoResponse));

        let result = board.port_up(Port::Downstream(1));

        assert!(matches!(result, Err(Error::NoResponse)));
    }
}
