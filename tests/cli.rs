// SPDX-License-Identifier: Apache-2.0
//! End-to-end tests that run the built binary.
//!
//! These cover the process entry point: exit codes, and which of stdout and
//! stderr a message lands on. Only commands that do not switch anything are
//! used, so the tests are safe to run with a board attached.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ykush3cmd"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn without_arguments_the_help_goes_to_stdout_and_succeeds() {
    let output = run(&[]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("ykush3cmd "));
    assert!(stdout(&output).contains("--i2c-read"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn the_help_option_prints_the_same_help() {
    for flag in ["-h", "--help"] {
        let output = run(&[flag]);

        assert!(output.status.success(), "{flag}");
        assert!(
            stdout(&output).contains("Switch the ports of a YKUSH3 hub"),
            "{flag}"
        );
    }
}

#[test]
fn the_version_option_prints_only_the_version() {
    let output = run(&["-v"]);

    assert!(output.status.success());
    assert!(stdout(&output).starts_with("ykush3cmd "));
    assert_eq!(stdout(&output).lines().count(), 1);
}

#[test]
fn an_unknown_option_fails_with_a_usage_message_on_stderr() {
    let output = run(&["-x"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("Unknown option -x"));
    assert!(stderr(&output).contains("Usage:"));
    assert!(stdout(&output).is_empty());
}

#[test]
fn an_invalid_value_fails_with_a_usage_message() {
    let output = run(&["-u", "9"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("Invalid port number '9'"));
}

#[test]
fn an_unknown_serial_number_fails_without_a_usage_message() {
    let output = run(&["-s", "no-such-board", "-g", "1"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("No YKUSH3 board with serial number no-such-board found"));
    // A board that is not there is not a usage error, so no usage text.
    assert!(!stderr(&output).contains("Usage:"));
}

#[test]
fn the_board_name_prefix_of_the_cpp_application_is_accepted() {
    let output = run(&["ykush3", "-h"]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("Switch the ports of a YKUSH3 hub"));
}
