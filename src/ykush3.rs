// SPDX-License-Identifier: Apache-2.0
//! YKUSH3 protocol layer.
//!
//! Report layout, opcodes and answer handling follow `src/ykush3/ykush3.cpp` of
//! the C++ application, so both tools speak the same wire protocol.

use std::fmt;

use crate::device::{report, Board, Report, Transport};
use crate::error::{Error, Result};

/// Largest payload that fits into a single I2C transfer report.
pub const I2C_MAX_BYTES: usize = 60;

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
    ///
    /// A number outside 1 to 3 is rejected rather than masked: unchecked it
    /// would bleed into the opcode — 0x42 would turn a switching command into
    /// BOOTLOADER — and masked it would silently address a different port.
    /// The command line never produces such a value, but a library caller can.
    fn code(self) -> Result<u8> {
        match self {
            Port::Downstream(n) if (1..=3).contains(&n) => Ok(n),
            Port::Downstream(n) => Err(Error::Usage(format!(
                "There is no downstream port {n}, the board has 1 to 3"
            ))),
            Port::External => Ok(0x4),
            Port::All => Ok(0xa),
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
    pub const I2C_CONFIG: u8 = 0x51;
    pub const I2C_TRANSFER: u8 = 0x52;
    pub const RESET: u8 = 0x55;
    pub const VERSION: u8 = 0x61;

    /// First byte of a successful answer.
    pub const ACK: u8 = 0x01;
    /// Status of an I2C transfer: the board is not in master mode.
    pub const I2C_NOT_MASTER: u8 = 0x02;
    /// Status of an I2C transfer: the transfer on the bus failed.
    pub const I2C_BUS_ERROR: u8 = 0x03;
}

/// Firmware versions before 1.1.0 do not know the version command.
const LEGACY_FIRMWARE: Version = Version {
    major: 1,
    minor: 0,
    patch: 0,
};
const LEGACY_BOOTLOADER: Version = Version {
    major: 0,
    minor: 10,
    patch: 0,
};

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

    /// The transport underneath, so a test can inspect what was sent.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Powers a port up.
    pub fn port_up(&self, port: Port) -> Result<()> {
        self.request_acked(&[op::PORT_UP | port.code()?])?;
        Ok(())
    }

    /// Powers a port down.
    pub fn port_down(&self, port: Port) -> Result<()> {
        self.request_acked(&[op::PORT_DOWN | port.code()?])?;
        Ok(())
    }

    /// Reads the switching state of a single port.
    pub fn port_status(&self, port: Port) -> Result<PortStatus> {
        if port == Port::All {
            return Err(Error::Usage(
                "The switching state can only be read for a single port".into(),
            ));
        }

        let resp = self.request_acked(&[op::PORT_STATUS | port.code()?])?;

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
        expect_ack(&resp, op::IO_READ)?;
        Ok(resp[3])
    }

    /// Drives a GPIO pin high or low.
    pub fn write_io(&self, gpio: u8, high: bool) -> Result<()> {
        self.request_acked(&[op::IO_WRITE, gpio, u8::from(high)])?;
        Ok(())
    }

    /// Enables or disables the GPIO control interface. Takes effect on the next
    /// reset or power-on of the board.
    pub fn gpio_control(&self, enable: bool) -> Result<()> {
        self.request_acked(&[op::IO_CONTROL, u8::from(enable)])?;
        Ok(())
    }

    /// Configures the power-on default state of a port.
    pub fn config_port(&self, port: Port, state: PowerOnState) -> Result<()> {
        if port == Port::All {
            return Err(Error::Usage(
                "The power-on state can only be configured for a single port".into(),
            ));
        }

        self.request_acked(&[op::PORT_CONFIG, port.code()?, state.code()])?;
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

    /// Enables or disables I2C slave mode.
    pub fn i2c_slave(&self, enable: bool) -> Result<()> {
        self.i2c_config(0x01, u8::from(enable))
    }

    /// Enables or disables I2C master mode.
    pub fn i2c_master(&self, enable: bool) -> Result<()> {
        self.i2c_config(0x02, u8::from(enable))
    }

    /// Sets the I2C slave address of the board.
    pub fn i2c_set_address(&self, address: u8) -> Result<()> {
        self.i2c_config(0x03, address)
    }

    /// Writes bytes to an I2C slave device.
    ///
    /// The board does not report a missing acknowledge from the slave: on an
    /// empty bus the write is reported as successful.
    pub fn i2c_write(&self, address: u8, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Err(Error::Usage("No data to write".into()));
        }
        if data.len() > I2C_MAX_BYTES {
            return Err(Error::Usage(format!(
                "At most {I2C_MAX_BYTES} bytes can be written in one transfer"
            )));
        }

        let mut req = vec![op::I2C_TRANSFER, 0x01, address, data.len() as u8];
        req.extend_from_slice(data);

        let resp = self.request(&req)?;
        expect_i2c_ack(&resp)
    }

    /// Reads bytes from an I2C slave device.
    ///
    /// An absent slave is not reported as an error: the board returns the idle
    /// bus level, so every byte reads back as `0xff`.
    pub fn i2c_read(&self, address: u8, len: u8) -> Result<Vec<u8>> {
        if usize::from(len) > I2C_MAX_BYTES {
            return Err(Error::Usage(format!(
                "At most {I2C_MAX_BYTES} bytes can be read in one transfer"
            )));
        }

        let resp = self.request(&[op::I2C_TRANSFER, 0x02, address, len])?;
        expect_i2c_ack(&resp)?;

        // The board reports how many bytes it actually got from the slave.
        let read = usize::from(resp[2]).min(I2C_MAX_BYTES);
        Ok(resp[3..3 + read].to_vec())
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

    fn i2c_config(&self, selector: u8, value: u8) -> Result<()> {
        let resp = self.request(&[op::I2C_CONFIG, selector, value])?;
        expect_ack(&resp, op::I2C_CONFIG)
    }

    /// Sends a command and returns the answer.
    fn request(&self, payload: &[u8]) -> Result<Report> {
        self.transport.transfer(&report(payload))
    }

    /// Sends a command and returns the answer, requiring the ACK status byte.
    ///
    /// For the port and GPIO commands the C++ application never looks at the
    /// answer, so a rejected command passes as a success there. The board does
    /// set the status byte, which is checked here; what follows it differs by
    /// command and is left to the caller.
    fn request_acked(&self, payload: &[u8]) -> Result<Report> {
        let resp = self.request(payload)?;
        if resp[0] != op::ACK {
            return Err(Error::Device(format!(
                "The board rejected the command (answer 0x{:02x})",
                resp[0]
            )));
        }
        Ok(resp)
    }

    /// Sends a command that the board does not answer.
    fn notify(&self, payload: &[u8]) -> Result<()> {
        self.transport.send(&report(payload))
    }
}

fn expect_ack(resp: &Report, opcode: u8) -> Result<()> {
    if resp[0] == op::ACK && resp[1] == opcode {
        return Ok(());
    }

    Err(Error::Device(format!(
        "The board rejected the command (answer 0x{:02x} 0x{:02x})",
        resp[0], resp[1]
    )))
}

/// Checks the answer to an I2C transfer, whose status byte carries a reason.
fn expect_i2c_ack(resp: &Report) -> Result<()> {
    if resp[1] != op::I2C_TRANSFER {
        return Err(Error::Device(format!(
            "Unexpected answer to an I2C command (0x{:02x} 0x{:02x})",
            resp[0], resp[1]
        )));
    }

    match resp[0] {
        op::ACK => Ok(()),
        op::I2C_NOT_MASTER => Err(Error::Device(
            "The board is not in I2C master mode, enable it with --i2c-master enable".into(),
        )),
        op::I2C_BUS_ERROR => Err(Error::Device("I2C transmission error".into())),
        status => Err(Error::Device(format!(
            "The board reported an unknown I2C status (0x{status:02x})"
        ))),
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

    /// Reports that the board acknowledges without further payload.
    const ACK_I2C_CONFIG: [u8; 2] = [0x01, 0x51];
    const ACK_I2C_TRANSFER: [u8; 2] = [0x01, 0x52];

    /// Message of a command that was expected to fail.
    fn message<T: fmt::Debug>(result: Result<T>) -> String {
        result.expect_err("command should have failed").to_string()
    }

    // -- port switching ---------------------------------------------------

    #[test]
    fn port_up_sends_the_opcodes_of_the_cpp_application() {
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
    fn port_down_sends_the_opcodes_of_the_cpp_application() {
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
            // The C++ application does not decode the external port at all.
            (0x14, PortStatus { port: 4, on: true }),
            (0x04, PortStatus { port: 4, on: false }),
        ] {
            let (result, _) = exchange(&[0x01, answer], |b| b.port_status(Port::Downstream(1)), 1);

            assert_eq!(result.unwrap(), expected, "answer 0x{answer:02x}");
        }
    }

    #[test]
    fn port_status_rejects_an_unexpected_answer() {
        let (result, _) = exchange(&[0x01, 0x55], |b| b.port_status(Port::Downstream(1)), 1);

        assert!(matches!(result, Err(Error::Device(_))));
    }

    #[test]
    fn a_state_change_the_board_does_not_acknowledge_is_an_error() {
        // Every command that changes state must surface a missing ACK instead
        // of treating whatever came back as a success.
        let rejected: Vec<(&str, Result<()>)> = vec![
            (
                "port_up",
                exchange(&[0x00], |b| b.port_up(Port::Downstream(1)), 1).0,
            ),
            (
                "port_down",
                exchange(&[0x00], |b| b.port_down(Port::All), 1).0,
            ),
            (
                "config_port",
                exchange(
                    &[0x00],
                    |b| b.config_port(Port::Downstream(2), PowerOnState::On),
                    3,
                )
                .0,
            ),
            ("write_io", exchange(&[0x00], |b| b.write_io(1, true), 3).0),
            (
                "gpio_control",
                exchange(&[0x00], |b| b.gpio_control(true), 2).0,
            ),
        ];

        for (name, result) in rejected {
            assert!(matches!(result, Err(Error::Device(_))), "{name}");
        }
    }

    #[test]
    fn an_out_of_range_port_number_is_rejected_before_anything_is_sent() {
        // The command line cannot produce these; a library caller can. Masked
        // or passed through, 18 would address port 2 and 0x45 would reset the
        // board.
        let board = Ykush3::with_transport(FakeBoard::mute());

        for n in [0, 4, 18, 0x45] {
            assert!(
                matches!(board.port_up(Port::Downstream(n)), Err(Error::Usage(_))),
                "port_up({n})"
            );
            assert!(
                matches!(board.port_status(Port::Downstream(n)), Err(Error::Usage(_))),
                "port_status({n})"
            );
        }
        assert_eq!(board.transport.sent_count(), 0, "nothing must be sent");
    }

    #[test]
    fn a_port_status_the_board_does_not_acknowledge_is_an_error() {
        // 0x11 in the second byte would decode as "port 1 on" — without the
        // status byte check the rejection would read as a valid state.
        let (result, _) = exchange(&[0x00, 0x11], |b| b.port_status(Port::Downstream(1)), 1);

        assert!(matches!(result, Err(Error::Device(_))));
    }

    #[test]
    fn a_gpio_read_answered_with_the_wrong_opcode_is_an_error() {
        let (no_ack, _) = exchange(&[0x00, 0x30, 0x02, 0x01], |b| b.read_io(2), 2);
        let (wrong_echo, _) = exchange(&[0x01, 0x41, 0x02, 0x01], |b| b.read_io(2), 2);

        assert!(matches!(no_ack, Err(Error::Device(_))));
        assert!(matches!(wrong_echo, Err(Error::Device(_))));
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

    // -- gpio -------------------------------------------------------------

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

    // -- configuration ----------------------------------------------------

    #[test]
    fn config_port_encodes_port_and_power_on_state() {
        for (port, state, expected) in [
            (Port::Downstream(1), PowerOnState::Off, [0x41, 0x01, 0x00]),
            (Port::Downstream(2), PowerOnState::On, [0x41, 0x02, 0x01]),
            (
                Port::Downstream(3),
                PowerOnState::Persist,
                [0x41, 0x03, 0x02],
            ),
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

    // -- reboot commands --------------------------------------------------

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

    // -- i2c --------------------------------------------------------------

    #[test]
    fn i2c_modes_are_configured_by_selector() {
        let (_, slave_on) = exchange(&ACK_I2C_CONFIG, |b| b.i2c_slave(true), 3);
        let (_, slave_off) = exchange(&ACK_I2C_CONFIG, |b| b.i2c_slave(false), 3);
        let (_, master_on) = exchange(&ACK_I2C_CONFIG, |b| b.i2c_master(true), 3);
        let (_, address) = exchange(&ACK_I2C_CONFIG, |b| b.i2c_set_address(0x2a), 3);

        assert_eq!(slave_on, vec![0x51, 0x01, 0x01]);
        assert_eq!(slave_off, vec![0x51, 0x01, 0x00]);
        assert_eq!(master_on, vec![0x51, 0x02, 0x01]);
        assert_eq!(address, vec![0x51, 0x03, 0x2a]);
    }

    #[test]
    fn i2c_config_reports_a_rejected_command() {
        let (result, _) = exchange(&[0x00, 0x51], |b| b.i2c_slave(true), 3);

        assert!(matches!(result, Err(Error::Device(_))));
    }

    #[test]
    fn i2c_write_carries_address_length_and_payload() {
        let (result, sent) = exchange(
            &ACK_I2C_TRANSFER,
            |b| b.i2c_write(0x20, &[0xaa, 0xbb, 0xcc]),
            7,
        );

        assert!(result.is_ok());
        assert_eq!(sent, vec![0x52, 0x01, 0x20, 0x03, 0xaa, 0xbb, 0xcc]);
    }

    #[test]
    fn i2c_write_rejects_more_than_the_report_holds() {
        let board = Ykush3::with_transport(FakeBoard::mute());

        let result = board.i2c_write(0x20, &[0xff; I2C_MAX_BYTES + 1]);

        assert!(matches!(result, Err(Error::Usage(_))));
        assert_eq!(board.transport.sent_count(), 0);
    }

    #[test]
    fn i2c_write_needs_data() {
        let board = Ykush3::with_transport(FakeBoard::mute());

        assert!(matches!(board.i2c_write(0x20, &[]), Err(Error::Usage(_))));
    }

    #[test]
    fn i2c_read_requests_and_returns_the_bytes() {
        let (result, sent) = exchange(
            &[0x01, 0x52, 0x03, 0xde, 0xad, 0xbe, 0xef],
            |b| b.i2c_read(0x20, 3),
            4,
        );

        assert_eq!(sent, vec![0x52, 0x02, 0x20, 0x03]);
        // Only the three bytes the board reports, not the trailing 0xef.
        assert_eq!(result.unwrap(), vec![0xde, 0xad, 0xbe]);
    }

    #[test]
    fn i2c_read_sends_the_length_as_a_single_byte() {
        // The C++ dec2bin() writes one byte per decimal digit and corrupts the
        // report for lengths of ten bytes and more.
        let (_, sent) = exchange(&[0x01, 0x52, 0x00], |b| b.i2c_read(0x20, 12), 4);

        assert_eq!(sent, vec![0x52, 0x02, 0x20, 12]);
    }

    #[test]
    fn i2c_read_rejects_more_than_the_report_holds() {
        let board = Ykush3::with_transport(FakeBoard::mute());

        let result = board.i2c_read(0x20, 61);

        assert!(matches!(result, Err(Error::Usage(_))));
        assert_eq!(board.transport.sent_count(), 0);
    }

    #[test]
    fn an_i2c_transfer_names_the_reason_it_failed() {
        // The status byte values are the ones the board answers with; 0x02 was
        // observed on firmware 1.5.0 with master mode switched off.
        let (not_master, _) = exchange(&[0x02, 0x52], |b| b.i2c_write(0x20, &[0x01]), 5);
        let (bus_error, _) = exchange(&[0x03, 0x52], |b| b.i2c_read(0x20, 2), 4);
        let (unknown, _) = exchange(&[0x7f, 0x52], |b| b.i2c_read(0x20, 2), 4);

        assert!(message(not_master).contains("not in I2C master mode"));
        assert!(message(bus_error).contains("I2C transmission error"));
        assert!(message(unknown).contains("0x7f"));
    }

    #[test]
    fn an_answer_to_the_wrong_command_is_reported_as_such() {
        let (result, _) = exchange(&[0x01, 0x41], |b| b.i2c_read(0x20, 2), 4);

        assert!(message(result).contains("Unexpected answer"));
    }

    // -- versions ---------------------------------------------------------

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

    // -- transport errors -------------------------------------------------

    #[test]
    fn a_transport_error_reaches_the_caller() {
        let board = Ykush3::with_transport(FakeBoard::failing(Error::NoResponse));

        let result = board.port_up(Port::Downstream(1));

        assert!(matches!(result, Err(Error::NoResponse)));
    }

    /// Needs a YKUSH3 attached.
    /// Run with `cargo test -- --ignored --test-threads=1`.
    mod hardware {
        use super::*;

        /// Checks that the firmware really sets the ACK byte on a switching
        /// command, which `request_acked` relies on. The port is set to the
        /// state it already has, so the test changes nothing.
        #[test]
        #[ignore = "needs a YKUSH3 attached"]
        fn a_switching_command_is_acknowledged_by_the_board() {
            let board = Ykush3::open(None).expect("the first board should open");
            let port = Port::Downstream(1);

            let status = board
                .port_status(port)
                .expect("status should be acknowledged");

            if status.on {
                board.port_up(port).expect("port up should be acknowledged");
            } else {
                board
                    .port_down(port)
                    .expect("port down should be acknowledged");
            }
        }
    }
}
