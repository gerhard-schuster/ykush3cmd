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

#[cfg(test)]
mod fake;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use cli::Command;
use error::{Error, Result};

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
    match cli::parse(args)? {
        Command::Help => help::print_all(out, program),
        Command::Version => help::print_version(out),
        Command::List => print_boards(out, &device::list()?),
    }
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
        let result = run("ykush3cmd", &["-x".to_owned()], &mut Vec::new());

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
    fn the_program_name_is_taken_from_the_invocation_path() {
        assert_eq!(program_name(&["/usr/bin/ykush3cmd".to_owned()]), "ykush3cmd");
        assert_eq!(program_name(&[]), "ykush3cmd");
    }
}
