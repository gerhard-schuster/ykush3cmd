// SPDX-License-Identifier: Apache-2.0
//! Command line parsing.
//!
//! The grammar of the C++ application is kept as it is: a token starting with a
//! dash opens an option, every following token that does not start with a dash
//! is a parameter of that option. Options such as `-on` or `-off` are therefore
//! single options and not a group of short flags.
//!
//! Parsing produces a [`Command`] and touches no hardware, so the whole command
//! line surface can be tested on its own.

use crate::error::{Error, Result};
use crate::ykush3::{Port, PowerOnState};

/// What the user asked the application to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    List,
    PortUp(Port),
    PortDown(Port),
    PortStatus(Port),
    Config(Port, PowerOnState),
    ReadIo(u8),
    WriteIo(u8, bool),
    GpioControl(bool),
    Reset,
    Bootloader,
    FirmwareVersion,
    BootloaderVersion,
}

/// A command together with the board it is addressed to.
#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub serial: Option<String>,
    pub command: Command,
}

/// One option of the command line together with its parameters.
#[derive(Debug, PartialEq, Eq)]
struct Opt {
    name: String,
    params: Vec<String>,
}

impl Opt {
    /// Parameter at `index`, or a usage error naming what was expected.
    fn param(&self, index: usize, expected: &str) -> Result<&str> {
        self.params
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| Error::Usage(format!("Option {} expects {expected}", self.name)))
    }
}

/// Interprets the arguments, without the program name.
pub fn parse(args: &[String]) -> Result<Invocation> {
    let opts = split(args);

    let Some(first) = opts.first() else {
        return Ok(Invocation {
            serial: None,
            command: Command::Help,
        });
    };

    // The serial number selects the board for whichever command follows, so it
    // is picked up before the command itself.
    let serial = match opts.iter().find(|o| o.name == "-s") {
        Some(opt) => Some(opt.param(0, "a serial number")?.to_owned()),
        None => None,
    };

    for opt in &opts {
        if opt.name == "-s" {
            continue;
        }
        if let Some(command) = command(opt)? {
            return Ok(Invocation { serial, command });
        }
    }

    // Nothing on the line was a command.
    Err(Error::Usage(if opts.len() == 1 && first.name == "-s" {
        "No command given".to_owned()
    } else {
        format!("Unknown option {}", first.name)
    }))
}

/// Maps one option to a command. `None` for an option that is not a command, so
/// the caller can keep looking.
fn command(opt: &Opt) -> Result<Option<Command>> {
    let command = match opt.name.as_str() {
        "-h" | "--help" => Command::Help,
        "-v" | "--version" => Command::Version,
        "-l" => Command::List,

        "-u" => Command::PortUp(port(opt.param(0, "a port number")?)?),
        "-d" => Command::PortDown(port(opt.param(0, "a port number")?)?),
        "-g" => Command::PortStatus(port(opt.param(0, "a port number")?)?),
        "-on" => Command::PortUp(Port::External),
        "-off" => Command::PortDown(Port::External),

        "-c" => Command::Config(
            port(opt.param(0, "a port number")?)?,
            power_on_state(opt.param(1, "a configuration value")?)?,
        ),

        "-r" => Command::ReadIo(gpio(opt.param(0, "a GPIO number")?)?),
        "-w" => Command::WriteIo(
            gpio(opt.param(0, "a GPIO number")?)?,
            level(opt.param(1, "a value of 0 or 1")?)?,
        ),
        "--gpio" => {
            Command::GpioControl(enable_disable(opt.param(0, "enable or disable")?, "--gpio")?)
        }

        "--reset" => Command::Reset,
        "--boot" => Command::Bootloader,
        "--firmware-version" => Command::FirmwareVersion,
        "--bootloader-version" => Command::BootloaderVersion,

        _ => return Ok(None),
    };

    Ok(Some(command))
}

/// Port of a switching command, `1|2|3|4|e|a`.
fn port(value: &str) -> Result<Port> {
    match value {
        "1" => Ok(Port::Downstream(1)),
        "2" => Ok(Port::Downstream(2)),
        "3" => Ok(Port::Downstream(3)),
        // The C++ application spells the external port `4` for switching and
        // `e` for configuration. Both spellings are accepted everywhere.
        "4" | "e" => Ok(Port::External),
        "a" => Ok(Port::All),
        _ => Err(Error::Usage(format!(
            "Invalid port number '{value}', expected 1, 2, 3, 4 (external 5V), e or a (all)"
        ))),
    }
}

/// Splits the arguments into options. A leading board name is skipped, so both
/// `ykush3cmd -u 1` and the `ykushcmd ykush3 -u 1` spelling work.
fn split(args: &[String]) -> Vec<Opt> {
    let mut opts: Vec<Opt> = Vec::new();

    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 {
            opts.push(Opt {
                name: arg.clone(),
                params: Vec::new(),
            });
        } else if let Some(last) = opts.last_mut() {
            last.params.push(arg.clone());
        }
    }

    opts
}

/// GPIO pin number, `1|2|3`.
fn gpio(value: &str) -> Result<u8> {
    match value {
        "1" => Ok(1),
        "2" => Ok(2),
        "3" => Ok(3),
        _ => Err(Error::Usage(format!(
            "Invalid GPIO number '{value}', expected 1, 2 or 3"
        ))),
    }
}

/// Logic level of a GPIO pin, `0|1`.
fn level(value: &str) -> Result<bool> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(Error::Usage(format!(
            "Invalid GPIO value '{value}', expected 0 or 1"
        ))),
    }
}

/// Power-on default of a port, `0|1|2`.
fn power_on_state(value: &str) -> Result<PowerOnState> {
    match value {
        "0" => Ok(PowerOnState::Off),
        "1" => Ok(PowerOnState::On),
        "2" => Ok(PowerOnState::Persist),
        _ => Err(Error::Usage(format!(
            "Invalid configuration value '{value}', expected 0 (off), 1 (on) or 2 (persistent)"
        ))),
    }
}

/// `enable` or `disable`.
fn enable_disable(value: &str, option: &str) -> Result<bool> {
    match value {
        "enable" => Ok(true),
        "disable" => Ok(false),
        _ => Err(Error::Usage(format!(
            "Invalid value '{value}' for {option}, expected enable or disable"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    /// Parses a command line and returns the command.
    fn cmd(line: &str) -> Command {
        parse(&args(line)).expect("command line should parse").command
    }

    /// Message of a command line that is expected to be rejected. A usage error
    /// displays as its bare message, so the caller can match on the text.
    fn usage_error(line: &str) -> String {
        parse(&args(line))
            .expect_err("command line should be rejected")
            .to_string()
    }

    #[test]
    fn options_collect_their_parameters() {
        let opts = split(&args("-c 1 2"));

        assert_eq!(
            opts,
            vec![Opt {
                name: "-c".into(),
                params: vec!["1".into(), "2".into()],
            }]
        );
    }

    #[test]
    fn a_leading_board_name_is_ignored() {
        assert_eq!(cmd("ykush3 -h"), Command::Help);
    }

    #[test]
    fn no_arguments_show_the_help() {
        assert_eq!(cmd(""), Command::Help);
    }

    #[test]
    fn an_unknown_option_is_rejected() {
        assert_eq!(usage_error("-x"), "Unknown option -x");
    }

    #[test]
    fn the_serial_number_selects_the_board() {
        let inv = parse(&args("-s YK00001 -d 2")).unwrap();

        assert_eq!(inv.serial.as_deref(), Some("YK00001"));
        assert_eq!(inv.command, Command::PortDown(Port::Downstream(2)));
    }

    #[test]
    fn the_serial_number_may_follow_the_command() {
        // The C++ application executes the first command it sees and would miss
        // a serial number given afterwards.
        let inv = parse(&args("-d 2 -s YK00001")).unwrap();

        assert_eq!(inv.serial.as_deref(), Some("YK00001"));
        assert_eq!(inv.command, Command::PortDown(Port::Downstream(2)));
    }

    #[test]
    fn without_a_serial_number_no_board_is_selected() {
        assert_eq!(parse(&args("-l")).unwrap().serial, None);
    }

    #[test]
    fn a_serial_number_alone_is_not_a_command() {
        assert_eq!(usage_error("-s YK00001"), "No command given");
    }

    #[test]
    fn a_serial_number_option_needs_a_value() {
        assert_eq!(usage_error("-s -d 1"), "Option -s expects a serial number");
    }

    #[test]
    fn port_numbers_map_to_ports() {
        assert_eq!(cmd("-u 1"), Command::PortUp(Port::Downstream(1)));
        assert_eq!(cmd("-u 3"), Command::PortUp(Port::Downstream(3)));
        assert_eq!(cmd("-u a"), Command::PortUp(Port::All));
        assert_eq!(cmd("-d 2"), Command::PortDown(Port::Downstream(2)));
        assert_eq!(cmd("-g 1"), Command::PortStatus(Port::Downstream(1)));
    }

    #[test]
    fn multi_character_short_options_stay_whole() {
        assert_eq!(cmd("-on"), Command::PortUp(Port::External));
        assert_eq!(cmd("-off"), Command::PortDown(Port::External));
    }

    #[test]
    fn the_external_port_has_two_spellings() {
        assert_eq!(cmd("-u 4"), Command::PortUp(Port::External));
        assert_eq!(cmd("-u e"), Command::PortUp(Port::External));
        assert_eq!(
            cmd("-c e 1"),
            Command::Config(Port::External, PowerOnState::On)
        );
        assert_eq!(
            cmd("-c 4 1"),
            Command::Config(Port::External, PowerOnState::On)
        );
    }

    #[test]
    fn the_power_on_state_is_parsed() {
        assert_eq!(
            cmd("-c 1 0"),
            Command::Config(Port::Downstream(1), PowerOnState::Off)
        );
        assert_eq!(
            cmd("-c 2 1"),
            Command::Config(Port::Downstream(2), PowerOnState::On)
        );
        assert_eq!(
            cmd("-c 3 2"),
            Command::Config(Port::Downstream(3), PowerOnState::Persist)
        );
    }

    #[test]
    fn an_invalid_power_on_state_is_rejected() {
        assert!(usage_error("-c 1 5").starts_with("Invalid configuration value '5'"));
        assert_eq!(usage_error("-c 1"), "Option -c expects a configuration value");
    }

    #[test]
    fn gpio_commands_are_parsed() {
        assert_eq!(cmd("-r 2"), Command::ReadIo(2));
        assert_eq!(cmd("-w 1 1"), Command::WriteIo(1, true));
        assert_eq!(cmd("-w 3 0"), Command::WriteIo(3, false));
        assert_eq!(cmd("--gpio enable"), Command::GpioControl(true));
        assert_eq!(cmd("--gpio disable"), Command::GpioControl(false));
    }

    #[test]
    fn invalid_gpio_arguments_are_rejected() {
        assert!(usage_error("-r 4").starts_with("Invalid GPIO number '4'"));
        assert!(usage_error("-w 1 2").starts_with("Invalid GPIO value '2'"));
        assert!(usage_error("--gpio on").starts_with("Invalid value 'on' for --gpio"));
    }

    #[test]
    fn an_invalid_port_number_is_rejected() {
        assert!(usage_error("-u 9").starts_with("Invalid port number '9'"));
        assert!(usage_error("-u x").starts_with("Invalid port number 'x'"));
    }

    #[test]
    fn a_port_command_needs_a_port_number() {
        assert_eq!(usage_error("-u"), "Option -u expects a port number");
    }

    #[test]
    fn the_first_command_on_the_line_wins() {
        // Same rule as in the C++ application, which returns after the first
        // command it recognises.
        assert_eq!(cmd("-u 1 -d 2"), Command::PortUp(Port::Downstream(1)));
    }

    #[test]
    fn the_plain_commands_are_parsed() {
        assert_eq!(cmd("-h"), Command::Help);
        assert_eq!(cmd("--help"), Command::Help);
        assert_eq!(cmd("-l"), Command::List);
        assert_eq!(cmd("--reset"), Command::Reset);
        assert_eq!(cmd("--boot"), Command::Bootloader);
        assert_eq!(cmd("--firmware-version"), Command::FirmwareVersion);
        assert_eq!(cmd("--bootloader-version"), Command::BootloaderVersion);
        assert_eq!(cmd("-v"), Command::Version);
        assert_eq!(cmd("--version"), Command::Version);
    }
}
