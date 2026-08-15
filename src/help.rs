// SPDX-License-Identifier: Apache-2.0
//! Help output.

use std::io::Write;

use crate::error::Result;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn print_version(out: &mut impl Write) -> Result<()> {
    writeln!(out, "ykush3cmd {VERSION}")?;
    Ok(())
}

pub fn print_usage(out: &mut impl Write, program: &str) -> Result<()> {
    writeln!(out, "Usage: {program} [-s SERIAL] COMMAND")?;
    writeln!(out, "Switch the ports of a YKUSH3 hub and read back its state.")?;
    Ok(())
}

pub fn print_options(out: &mut impl Write) -> Result<()> {
    write!(
        out,
        "
Ports. The number 4 addresses the separate 5V output, the letter a every
downstream port at once.

  -u PORT            power the port up            PORT = 1|2|3|4|a
  -d PORT            power the port down          PORT = 1|2|3|4|a
  -g PORT            report whether it is on      PORT = 1|2|3|4
  -on                shorthand for -u 4
  -off               shorthand for -d 4

Board.

  -l                 list the serial number of every attached board
  -s SERIAL          send the command to that board rather than to the
                     first one the system enumerates
  -v, --version      print the version of this program
  -h, --help         print this text

Exit status is 0 when the command went through and 1 when it did not.
"
    )?;
    Ok(())
}

pub fn print_all(out: &mut impl Write, program: &str) -> Result<()> {
    print_version(out)?;
    print_usage(out, program)?;
    print_options(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::fake::FailingWriter;

    #[test]
    fn the_help_names_the_version_and_the_usage() {
        let mut out = Vec::new();

        print_all(&mut out, "ykush3cmd").unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(text.contains(&format!("ykush3cmd {VERSION}")));
        assert!(text.contains("Usage: ykush3cmd"));
    }

    #[test]
    fn a_failing_writer_is_reported_instead_of_panicking() {
        // Every part on its own, so no early failure hides a later one.
        assert!(matches!(print_version(&mut FailingWriter), Err(Error::Io(_))));
        assert!(matches!(
            print_usage(&mut FailingWriter, "ykush3cmd"),
            Err(Error::Io(_))
        ));
        assert!(matches!(print_options(&mut FailingWriter), Err(Error::Io(_))));
        assert!(matches!(
            print_all(&mut FailingWriter, "ykush3cmd"),
            Err(Error::Io(_))
        ));
    }
}
