# Security review

| | |
|---|---|
| Subject | the Rust port, at the time still a branch inside the C++ repository |
| State | commit `6a4dca9`, range reviewed `master...rust-port-ykush3` |
| Scope | `src/`, `tests/`, `Cargo.toml`, `Cargo.lock` |
| Date | 2026-08-15 |
| Method | systematic reading of the diff and the sources along the categories listed below |

The unchanged C++ part of that repository was not part of the review.

**Result: no HIGH or MEDIUM severity findings.**

## Attack surface examined

| Category | Finding |
|---|---|
| Command injection | no `std::process::Command` in production code; only in `tests/cli.rs`, with `CARGO_BIN_EXE_*` and fixed arguments |
| Path traversal, file access | no file operations at all; `Path` is used solely for the basename of `argv[0]` (`src/main.rs`) |
| SQL, NoSQL, XXE, templating, deserialisation | not applicable — the only dependency is `hidapi 2.6.6` |
| Secrets, crypto | no keys, tokens or crypto calls; only the constants VID `0x04D8` and PID `0xF11B` |
| Format string injection | every `format!` and `writeln!` uses a literal with values as arguments, which is not injectable by construction |
| `unsafe` | not used anywhere in the port |

## Focus: the trust boundary to the USB device

The only input not controlled by the user is the 64 byte answer report from the board.
Handling it is sound against a hostile or faulty device:

- `Report` is a fixed `[u8; 64]` (`src/device.rs`), and every `transfer()` allocates a
  freshly zeroed buffer. A short read leaves zeros rather than remnants of an earlier
  answer, so nothing leaks between commands.
- The one length the device controls is clamped: `usize::from(resp[2]).min(I2C_MAX_BYTES)`
  ahead of the slice `resp[3..3 + read]` (`src/ykush3.rs`). Highest index 63 in a 64 byte
  buffer.
- Every other access to an answer (`resp[0]` through `resp[4]`) is at a constant index
  inside the buffer.
- No `unwrap()`, `expect()` or `panic!` in production code — every occurrence is inside a
  `#[cfg(test)]` module.

## Observations below the reporting bar

Neither of these is a finding; they are noted deliberately.

1. **Serial numbers were printed unchanged** (`src/main.rs`). A doctored USB device could
   put ANSI escape sequences in its serial number. The C++ original behaves identically
   (`src/yk_usb_device.cpp`), so the port introduced no new risk, and a terminal escape
   vector on its own does not carry a finding.
2. **Any local process can drive the board.** macOS hands a HID device to whoever opens it
   first, with no privilege required, so any program running as the user can switch ports
   or trigger the bootloader. That is a property of the platform and the device, not of
   this program, which has no place to add a check. Worth knowing rather than worth fixing.

## Compared with the C++ original

The port improves the position: the unchecked length handling on an I2C read and the report
corruption caused by `dec2bin()` are replaced by clamping and by single byte encoding
respectively.

## Addendum

Observation 1 was hardened after the review: `device::sanitize()` replaces every control
character as the serial number is read in. The filter sits at the system boundary, where the
string is taken from the device, rather than at the point of printing. The finding above
describes the state at the time of the review.

## Scope

A snapshot of the commit named above. Changes made to the code afterwards are not covered.
A review without findings is no proof of correctness — it says that nothing above the
reporting bar was found in the categories examined.
