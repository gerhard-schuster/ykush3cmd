// SPDX-License-Identifier: Apache-2.0
//! Test doubles.

/// A writer whose every write fails, to exercise the output error paths.
pub struct FailingWriter;

impl std::io::Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "writer closed",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn the_failing_writer_fails_on_write_but_not_on_flush() {
        assert!(FailingWriter.write(b"x").is_err());
        assert!(FailingWriter.flush().is_ok());
    }
}
