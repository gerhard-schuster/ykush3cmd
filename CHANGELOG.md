# Changelog

Notable changes, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.4.0 - 2026-09-24

The HID transport moves from `hidapi`, which carries a C library and compiles
it, to `async-hid`, which reaches IOKit from Rust. Building needs a Rust
toolchain and nothing else - no C compiler, no Xcode command line tools. The
nine hardware tests were run against a board before and after the move.

Two findings from the review of that new transport are fixed, the account of
where the protocol came from is corrected, and the transport is pinned to a
revision of the fork instead of a directory on one machine, so a clone of this
repository builds on its own.

### Changed

- The transport is `async-hid`. The library is asynchronous because that is how
  macOS delivers HID reports; the `Transport` trait and everything above it stay
  synchronous, so the change is confined to `device.rs`.
- The read timeout of five seconds is now built by racing the read against a
  timer, since the new library offers no timed read. A read that loses the race
  leaves the device usable.
- No leading report id byte is prepended any more. The new library takes the
  bare 64 byte report, which suits a board that uses unnumbered reports.
- The minimum supported Rust is 1.85, up from 1.74. `async-hid` moved to
  edition 2024, which no earlier compiler accepts. Nothing in this crate needs
  it; the floor comes from the dependency.
- The documentation now states where the protocol came from. It was implemented
  from Yepkit's published USB control interface and I2C references and checked
  against a board, not taken from the source of the C++ `ykushcmd`; the README,
  `NOTICE`, `doc/README.md` and the crate documentation had described this
  program as a port of that source. `NOTICE` also records that the two are built
  differently - a library behind a substitutable transport here, a single
  executable whose board classes inherit their HID access there. No code
  changed; what was wrong was the account of how the code came to be.

### Security

- `SECURITY-REVIEW-0.4.0.md` reviews the state after the transport change; the
  earlier `SECURITY-REVIEW.md` is kept as the record of the `hidapi` state. No
  HIGH or MEDIUM finding, one LOW, and a second LOW that was withdrawn after a
  closer look - the review carries an addendum saying what changed and why the
  withdrawn one does not stand.
- An answer that arrives after a read has timed out is no longer read as the
  answer to the next command. A timeout marks the board, and the next exchange
  empties the queue of the transport before it sends. The drain is bounded by a
  50 ms wait and by 16 reports. This was reachable only through the library: one
  run of the command line program performs one exchange and exits.
- `sanitize()` now replaces the bidirectional overrides and the zero width
  characters as well as the control characters. They cannot drive a terminal,
  but `U+202E` in a serial number reorders the line it is printed on and
  `U+200B` prints as nothing, which together are enough to show one board's
  serial number as another's. `U+FFFD` is still kept, so a lossy `argv`
  conversion still shows where the bad byte was.
- The transport is pinned to a revision of the fork rather than patched to a
  directory next to this repository, so a clone of this repository builds on its
  own and `Cargo.lock` records a revision anyone can check. It was a
  `[patch.crates-io]` pointing at `../async-hid` before, which resolved to
  whatever happened to be on that disk. The pin goes away once the fix is
  released on crates.io; the open pull request is sidit77/async-hid#46.

### Removed

- `Error::HidInit` - it reported a failure of the C library's global
  initialisation, which no longer exists. Library callers matching on it have to
  drop the arm; the variant can never be constructed.
- `LICENSE-hidapi-bsd.txt` and the licensing apparatus around it. Nothing links
  a third party C library any more, so nothing has to accompany a binary release
  beyond the Apache license and the notices.

## 0.3.0 - 2026-08-16

The protocol layer now holds every answer to the shape of the request it
belongs to, and the command line rejects what it used to skip silently. The
differences table in the README lists every deliberate divergence from the
C++ application in one place.

### Changed

- The board's answers are held to the shape of the request. A port status
  must describe the port that was asked about and a GPIO read the pin that
  was asked about; a version answer is either the exact empty shape of a
  legacy board or a properly acknowledged version - a garbled answer is an
  error rather than a made-up `1.0.0`; an I2C read claiming more bytes than
  requested is rejected instead of clamped. Each of these was taken at face
  value before, as the C++ application does.
- GPIO numbers are validated in the library, not only in the command line.
- Only the literal word `ykush3` is accepted as a leading board name - any
  other free-standing first word is an error instead of being skipped - and
  a second `-s` on one line is rejected instead of the first silently
  winning.
- `report()` is no longer part of the library's public interface, and
  `Error::source()` hands out the underlying error of an output failure.

## 0.2.0 - 2026-08-16

Hardening after a security audit, and a library underneath the command line.
Scripts that only look at the exit status get stricter behaviour on purpose:
a command the board rejects now exits with 1 instead of passing silently.

### Added

- A library target `ykush3` underneath the command line application, so the
  board can be scripted from Rust directly. The `FakeBoard` test double ships
  with it behind the `fake` feature: code built on the library can be tested
  without hardware, the same way this repository tests itself, while a build
  that does not ask for the double gets a library without test gear or
  panicking paths.

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
  crash, and an empty argument vector - which `execve` permits - shows the
  help instead of crashing.

### Security

- Control characters in a command line argument are replaced before an error
  message echoes the argument, closing the same terminal escape route that was
  already closed for serial numbers a device reports.
- The CI workflows now grant the token read access only, stop persisting
  credentials into the checkout, pin the actions to commits instead of movable
  tags, and pin the cargo-deny version so the audit tool no longer sits frozen
  in a stale cache.

## 0.1.0 - 2026-08-15

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
