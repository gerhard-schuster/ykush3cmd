// SPDX-License-Identifier: Apache-2.0
//! Control application for the Yepkit YKUSH3 switchable USB hub.
//!
//! Rust port of the YKUSH3 part of the Yepkit `ykushcmd` application.

// This program is written for macOS on Apple silicon, and has only ever been
// built and run there. Anywhere else it refuses to compile rather than produce
// a binary nobody has tried.
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
compile_error!("ykush3cmd is for macOS on Apple silicon");

mod cli;
mod device;
mod error;
mod help;
mod ykush3;

#[cfg(test)]
mod fake;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use cli::Command;
use device::Transport;
use error::{Error, Result};
use ykush3::Ykush3;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let program = program_name(&args);
    let mut out = io::stdout().lock();

    match run(&program, &args[1..], &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Usage(msg)) => {
            let _ = writeln!(io::stderr(), "{msg}\n");
            let _ = help::print_usage(&mut io::stderr(), &program);
            let _ = writeln!(
                io::stderr(),
                "\nRun '{program} -h' for the list of options."
            );
            ExitCode::FAILURE
        }
        Err(err) => {
            let _ = writeln!(io::stderr(), "{err}");
            ExitCode::FAILURE
        }
    }
}

fn run(program: &str, args: &[String], out: &mut impl Write) -> Result<()> {
    let invocation = cli::parse(args)?;

    match &invocation.command {
        // Commands that do not address a single board.
        Command::Help => help::print_all(out, program),
        Command::Version => help::print_version(out),
        Command::List => print_boards(out, &device::list()?),

        command => {
            let board = Ykush3::open(invocation.serial.as_deref())?;
            execute(&board, command, out)
        }
    }
}

/// Runs a command against an opened board and prints what it returns.
fn execute<T: Transport>(
    board: &Ykush3<T>,
    command: &Command,
    out: &mut impl Write,
) -> Result<()> {
    match command {
        Command::PortUp(port) => board.port_up(*port)?,
        Command::PortDown(port) => board.port_down(*port)?,
        Command::PortStatus(port) => writeln!(out, "{}", board.port_status(*port)?)?,

        Command::Config(port, state) => board.config_port(*port, *state)?,

        Command::ReadIo(gpio) => writeln!(out, "{}", board.read_io(*gpio)?)?,
        Command::WriteIo(gpio, high) => board.write_io(*gpio, *high)?,
        Command::GpioControl(enable) => board.gpio_control(*enable)?,

        Command::Reset => board.reset()?,
        Command::Bootloader => board.enter_bootloader()?,

        Command::FirmwareVersion => {
            writeln!(out, "Firmware {}", board.firmware_version()?)?
        }
        Command::BootloaderVersion => {
            writeln!(out, "Bootloader {}", board.bootloader_version()?)?
        }

        Command::I2cSlave(enable) => board.i2c_slave(*enable)?,
        Command::I2cMaster(enable) => board.i2c_master(*enable)?,
        Command::I2cSetAddress(address) => board.i2c_set_address(*address)?,
        Command::I2cWrite(address, data) => board.i2c_write(*address, data)?,
        Command::I2cRead(address, len) => {
            for byte in board.i2c_read(*address, *len)? {
                writeln!(out, "{byte}")?;
            }
        }

        // Handled by run() before a board is opened.
        Command::Help | Command::Version | Command::List => {
            unreachable!("command does not address a board")
        }
    }

    Ok(())
}

fn print_boards(out: &mut impl Write, serials: &[String]) -> Result<()> {
    writeln!(out, "YKUSH3 boards on this host:")?;
    if serials.is_empty() {
        writeln!(out, "  none")?;
    }
    for (i, serial) in serials.iter().enumerate() {
        writeln!(out, "  {:>2}  {serial}", i + 1)?;
    }

    Ok(())
}

fn program_name(args: &[String]) -> String {
    args.first()
        .map(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.clone())
        })
        .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeBoard;
    use crate::ykush3::Port;

    /// Runs a command against a board answering with `answer` and returns what
    /// was printed.
    fn output(answer: &[u8], command: Command) -> String {
        let board = Ykush3::with_transport(FakeBoard::answering(answer));
        let mut out = Vec::new();

        execute(&board, &command, &mut out).expect("command should succeed");

        String::from_utf8(out).expect("output should be utf-8")
    }

    #[test]
    fn the_port_state_is_printed_as_a_line() {
        assert_eq!(
            output(&[0x01, 0x11], Command::PortStatus(Port::Downstream(1))),
            "Port 1: on\n"
        );
        assert_eq!(
            output(&[0x01, 0x02], Command::PortStatus(Port::Downstream(2))),
            "Port 2: off\n"
        );
    }

    #[test]
    fn a_gpio_read_prints_the_pin_level() {
        // The C++ application only puts the level into the exit code.
        assert_eq!(output(&[0x01, 0x30, 0x01, 0x01], Command::ReadIo(1)), "1\n");
        assert_eq!(output(&[0x01, 0x30, 0x01, 0x00], Command::ReadIo(1)), "0\n");
    }

    #[test]
    fn versions_are_printed_with_their_kind() {
        assert_eq!(
            output(&[0x01, 0x61, 1, 4, 0], Command::FirmwareVersion),
            "Firmware 1.4.0\n"
        );
        assert_eq!(
            output(&[0x01, 0x61, 0, 10, 0], Command::BootloaderVersion),
            "Bootloader 0.10.0\n"
        );
    }

    #[test]
    fn an_i2c_read_prints_one_decimal_byte_per_line() {
        assert_eq!(
            output(&[0x01, 0x52, 0x03, 0x01, 0x0a, 0xff], Command::I2cRead(0x20, 3)),
            "1\n10\n255\n"
        );
    }

    #[test]
    fn switching_commands_print_nothing() {
        assert_eq!(output(&[0x01], Command::PortUp(Port::Downstream(1))), "");
        assert_eq!(output(&[0x01], Command::WriteIo(1, true)), "");
        assert_eq!(output(&[0x01, 0x51], Command::I2cSlave(true)), "");
    }

    #[test]
    fn a_failing_command_is_reported_and_prints_nothing() {
        let board = Ykush3::with_transport(FakeBoard::failing(Error::NoResponse));
        let mut out = Vec::new();

        let result = execute(&board, &Command::PortStatus(Port::Downstream(1)), &mut out);

        assert!(matches!(result, Err(Error::NoResponse)));
        assert!(out.is_empty());
    }

    #[test]
    fn the_help_names_the_program_it_was_called_as() {
        let mut out = Vec::new();

        run("ykushcmd", &[], &mut out).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Usage: ykushcmd"));
    }

    #[test]
    fn the_version_is_printed_on_its_own() {
        let mut out = Vec::new();

        run("ykush3cmd", &["-v".to_owned()], &mut out).unwrap();

        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("ykush3cmd {}\n", help::VERSION)
        );
    }

    #[test]
    fn an_unparsable_command_line_is_a_usage_error() {
        let result = run("ykush3cmd", &["-u".to_owned(), "9".to_owned()], &mut Vec::new());

        assert!(matches!(result, Err(Error::Usage(_))));
    }

    #[test]
    fn attached_boards_are_listed_with_their_serial_numbers() {
        let mut out = Vec::new();

        print_boards(&mut out, &["YK00001".to_owned(), "YK00002".to_owned()]).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("YKUSH3 boards on this host:"));
        assert!(text.contains("1  YK00001"));
        assert!(text.contains("2  YK00002"));
    }

    #[test]
    fn an_empty_board_list_says_so() {
        let mut out = Vec::new();

        print_boards(&mut out, &[]).unwrap();

        assert!(String::from_utf8(out).unwrap().contains("none"));
    }

    /// Needs the HID stack of the operating system, but no board.
    #[test]
    #[ignore = "needs the HID stack of the operating system"]
    fn listing_boards_reaches_the_hid_stack() {
        let mut out = Vec::new();

        run("ykush3cmd", &["-l".to_owned()], &mut out).expect("listing should work");

        assert!(String::from_utf8(out)
            .unwrap()
            .contains("YKUSH3 boards on this host"));
    }

    #[test]
    fn every_command_that_addresses_the_board_is_dispatched() {
        use crate::ykush3::PowerOnState;

        let answered: Vec<(Vec<u8>, Command)> = vec![
            (vec![0x01], Command::PortUp(Port::All)),
            (vec![0x01], Command::PortDown(Port::Downstream(2))),
            (
                vec![0x01],
                Command::Config(Port::Downstream(1), PowerOnState::Persist),
            ),
            (vec![0x01], Command::WriteIo(2, false)),
            (vec![0x01], Command::GpioControl(false)),
            (vec![0x01, 0x51], Command::I2cSlave(true)),
            (vec![0x01, 0x51], Command::I2cMaster(false)),
            (vec![0x01, 0x51], Command::I2cSetAddress(0xa6)),
            (vec![0x01, 0x52], Command::I2cWrite(0x20, vec![0x01])),
            (vec![0x01, 0x61, 1, 2, 0], Command::BootloaderVersion),
        ];

        for (answer, command) in answered {
            let board = Ykush3::with_transport(FakeBoard::answering(&answer));

            execute(&board, &command, &mut Vec::new())
                .unwrap_or_else(|err| panic!("{command:?} failed: {err}"));

            assert_eq!(board.transport().sent_count(), 1, "{command:?}");
        }

        // These two make the board reboot, so it never answers.
        for command in [Command::Reset, Command::Bootloader] {
            let board = Ykush3::with_transport(FakeBoard::mute());

            execute(&board, &command, &mut Vec::new())
                .unwrap_or_else(|err| panic!("{command:?} failed: {err}"));

            assert_eq!(board.transport().sent_count(), 1, "{command:?}");
        }
    }

    /// Needs a YKUSH3 attached.
    /// Run with `cargo test -- --ignored --test-threads=1`.
    #[test]
    #[ignore = "needs a YKUSH3 attached"]
    fn a_read_only_command_reaches_a_real_board() {
        let mut out = Vec::new();

        run("ykush3cmd", &["--firmware-version".to_owned()], &mut out).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("Firmware "), "{text:?}");
    }

    #[test]
    fn the_program_name_is_taken_from_the_invocation_path() {
        assert_eq!(program_name(&["/usr/bin/ykush3cmd".to_owned()]), "ykush3cmd");
        assert_eq!(program_name(&[]), "ykush3cmd");
    }
}
