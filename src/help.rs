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
