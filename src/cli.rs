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

/// What the user asked the application to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
}

/// One option of the command line together with its parameters.
#[derive(Debug, PartialEq, Eq)]
struct Opt {
    name: String,
    params: Vec<String>,
}

/// Interprets the arguments, without the program name.
pub fn parse(args: &[String]) -> Result<Command> {
    let opts = split(args);

    let Some(first) = opts.first() else {
        return Ok(Command::Help);
    };

    for opt in &opts {
        if let Some(command) = command(opt) {
            return Ok(command);
        }
    }

    // Nothing on the line was a command.
    Err(Error::Usage(format!("Unknown option {}", first.name)))
}

/// Maps one option to a command. `None` for an option that is not a command, so
/// the caller can keep looking.
fn command(opt: &Opt) -> Option<Command> {
    match opt.name.as_str() {
        "-h" | "--help" => Some(Command::Help),
        "-v" | "--version" => Some(Command::Version),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    /// Parses a command line and returns the command.
    fn cmd(line: &str) -> Command {
        parse(&args(line)).expect("command line should parse")
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
    fn the_plain_commands_are_parsed() {
        assert_eq!(cmd("-h"), Command::Help);
        assert_eq!(cmd("--help"), Command::Help);
        assert_eq!(cmd("-v"), Command::Version);
        assert_eq!(cmd("--version"), Command::Version);
    }
}
