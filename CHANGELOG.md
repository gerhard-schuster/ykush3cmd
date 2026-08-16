# Changelog

Notable changes, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- A library target `ykush3` underneath the command line application, so the
  board can be scripted from Rust directly. The `FakeBoard` test double ships
  as part of it, so code built on the library can be tested without hardware,
  the same way this repository tests itself.

### Changed

- Narrowed to macOS on Apple silicon, which is where this was written, built and
  run. Anywhere else the build now stops with a message saying so, rather than
  producing a binary nobody has tried.

### Fixed

- Port and GPIO commands now check the board's acknowledgement. Before, as in
  the C++ application, a rejected command still exited with 0 and a script had
  no way of noticing.
- An answer report shorter than the fixed 64 bytes is rejected instead of
  having its missing bytes read as zeros.
- An argument that is not valid Unicode ends in a usage error instead of a
  crash, and an empty argument vector — which `execve` permits — shows the
  help instead of crashing.

### Security

- Control characters in a command line argument are replaced before an error
  message echoes the argument, closing the same terminal escape route that was
  already closed for serial numbers a device reports.
- The CI workflows now grant the token read access only, stop persisting
  credentials into the checkout, pin the actions to commits instead of movable
  tags, and pin the cargo-deny version so the audit tool no longer sits frozen
  in a stale cache.

## 0.1.0 — 2026-08-15

First release. A control program for the Yepkit YKUSH3, written in Rust against
the USB protocol the vendor documents.

### Added

- Switching of the three downstream ports and the external 5V output, singly or
  all at once, and reading a port's state back
- Reading and writing the three GPIO pins, and handing them to the board as a
  control interface
- Configuring what a port does after a reset or a power cut
- Reset, entering the bootloader, and reading the firmware and bootloader
  versions
- I2C in both roles: as a slave the board takes switching commands from an
  outside master, as a master it carries transfers from the host onto the bus
- Listing attached boards and addressing one by serial number
- 81 tests that need no hardware, plus 6 that do: four want a board, two only
  the HID stack of the operating system

### Notes for anyone coming from the C++ application

The switches are the same and a leading `ykush3` is accepted, so existing
invocations keep working. The messages are worded differently, so a script that
parses output has to be adapted; exit codes are unchanged.

Behaviour that differs on purpose, each of them fixing something:

- the state of the external 5V port is decoded rather than reported as an error
- an I2C read longer than nine bytes no longer corrupts the report
- a GPIO read prints its value instead of returning it as the exit status
- hexadecimal arguments are accepted with and without the `0x` prefix
- `--reset` and `--boot` no longer wait for an answer the board never sends
- a serial number works wherever it appears on the line
- control characters in a serial number the device reports are replaced before
  the string reaches the terminal
