// SPDX-License-Identifier: Apache-2.0
//! Keeps text that enters the program from carrying terminal escapes.

/// Replaces control characters in a string from outside the program.
///
/// A serial number is whatever the USB descriptor says, argv is whatever the
/// caller put there, and both end up on the terminal. Without this, a device
/// or a calling script could hide an ANSI escape sequence in the text and
/// control what the terminal does with the rest of the line.
pub fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_serial_number_from_the_device_cannot_carry_terminal_escapes() {
        // What a hostile device could put into its USB descriptor.
        assert_eq!(sanitize("YK\x1b[2JEVIL"), "YK?[2JEVIL");
        assert_eq!(sanitize("YK\r\n\x07"), "YK???");
        assert_eq!(sanitize("YK\u{9b}31m"), "YK?31m");

        // An ordinary serial number is left alone.
        assert_eq!(sanitize("Y3N13808"), "Y3N13808");
        assert_eq!(sanitize("<unknown>"), "<unknown>");
    }

    #[test]
    fn an_argument_echoed_in_an_error_cannot_carry_terminal_escapes() {
        // What a calling script could pass and later see echoed on stderr.
        assert_eq!(sanitize("-\x1b[31mred"), "-?[31mred");
        assert_eq!(sanitize("5\x07"), "5?");

        // The replacement character from a lossy argv conversion is kept, so
        // the error message still shows where the bad byte was.
        assert_eq!(sanitize("YK\u{fffd}1"), "YK\u{fffd}1");
    }
}
