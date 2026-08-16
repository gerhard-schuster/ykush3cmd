# ykush3cmd

A Rust port of the YKUSH3 part of Yepkit's `ykushcmd`, **for macOS on Apple silicon**.
YKUSH3 only (VID `0x04D8`, PID `0xF11B`) — YKUSH, YKUSH2 and YKUSHXS are not covered.

1. [Building](#building)
2. [Using it](#using-it)
3. [The hardware](#the-hardware)
4. [USB control protocol](#usb-control-protocol)
5. [How the port is put together](#how-the-port-is-put-together)
6. [Tests](#tests)
7. [Checked against the hardware](#checked-against-the-hardware)
8. [Where it differs from the C++ original](#where-it-differs-from-the-c-original)
9. [Driving a YKUR relay board over I2C](#driving-a-ykur-relay-board-over-i2c)
10. [Documents and sources](#documents-and-sources)
11. [License](#license)

---

## Building

```
cargo build --release
```

The result is a single binary, `target/release/ykush3cmd`.

With a Rust toolchain (1.74 or newer) installed, building and installing is one step,
no clone needed:

```
cargo install --locked --tag v0.2.0 --git https://github.com/gerhard-schuster/ykush3cmd
```

This puts `ykush3cmd` into `~/.cargo/bin`. The `--locked` matters: without it, cargo
resolves the dependencies afresh instead of taking the versions pinned in `Cargo.lock` —
the ones the tests and the dependency audit actually ran against. The tag picks the
release; leave it off to build the tip of `master`.

**For macOS on Apple silicon.** That is where it was written, built and run, and the source
refuses to compile anywhere else rather than produce a binary nobody has tried.

The only dependency is the `hidapi` crate. It carries the HID library as C source and
reaches the device by way of IOKit, so nothing else has to be installed and no library
travels alongside the binary.

Reaching the device needs no special permission — the system hands it to any process that
asks. See [SECURITY-REVIEW.md](SECURITY-REVIEW.md) for what that means.

Every push builds and tests on macOS, see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## Using it

The **switches** are compatible with the original, and a leading `ykush3` is accepted and
ignored so existing invocations keep working:

```
ykush3cmd -u 1
ykush3cmd ykush3 -u 1        # the same thing
```

The **messages** deliberately are not compatible — they are worded independently, see
[License](#license). A script that picks apart the output of the original needs adapting;
a script that only looks at the exit status does not.

| Option | Effect |
|---|---|
| `-u 1\|2\|3\|4\|a` | power a port up (4 is the external 5V output, a is all of them) |
| `-d 1\|2\|3\|4\|a` | power a port down |
| `-g 1\|2\|3\|4` | report whether a port is on |
| `-on` / `-off` | switch the external 5V output |
| `-l` | list the attached boards |
| `-s <serial>` | address the board with that serial number |
| `-c 1\|2\|3\|e 0\|1\|2` | configure the power-on state (off / on / last state) |
| `-r 1\|2\|3` | read a GPIO pin |
| `-w 1\|2\|3 0\|1` | drive a GPIO pin |
| `--gpio enable\|disable` | hand the GPIO pins to the board as a control interface |
| `--i2c-slave enable\|disable` | I2C slave mode |
| `--i2c-master enable\|disable` | I2C master mode |
| `--i2c-set-address <hex>` | the address the board answers to as a slave |
| `--i2c-write <hex-addr> <byte>...` | write bytes to an I2C slave (60 at most) |
| `--i2c-read <hex-addr> <n>` | read n bytes from an I2C slave (60 at most) |
| `--reset` | restart the board |
| `--boot` | stop in the bootloader |
| `--firmware-version` / `--bootloader-version` | print the versions |
| `-v`, `--version` / `-h`, `--help` | version and help |

Exit status: `0` on success, `1` on failure — board not found, invalid input, or the board
rejecting the command.

```
$ ykush3cmd -l
YKUSH3 boards on this host:
   1  Y3N13808

$ ykush3cmd -s Y3N13808 -d 2      # cut power to port 2
$ ykush3cmd -g 2
Port 2: off
$ ykush3cmd -c 2 1                # port 2 comes up on the next reset
$ ykush3cmd --firmware-version
Firmware 1.5.0
```

## The hardware

Every figure below is from the vendor datasheet, see [Documents](#documents-and-sources).

### Layout

The YKUSH3 is a USB hub with power switches in front of its ports and a microcontroller to
drive them. Plugging it in makes **three** devices appear on the host: a USB 3.2 Gen1 hub, a
USB 2.0 hub, and the controller as a USB HID device, `04D8:F11B`. The controller sits on the
bus permanently and cannot be switched off, which is why the board still answers when every
port is dead.

Ports can be switched three ways: over **USB**, which is always active, over **GPIO**, and
over **I2C**. The latter two are disabled from the factory and have to be enabled once over
USB; that setting survives resets and power cuts.

### Downstream ports

| Item | Value |
|---|---|
| Count | 3, individually switched, USB 3.0 type A |
| What gets switched | VBUS **and** the data lines, SuperSpeed as well as USB 2.0 |
| Hub core | USB 3.2 Gen1 (5 Gbps) plus USB 2.0 (480/12/1.5 Mbps) |
| Battery charging | BC1.2: DCP, CDP (1.5 A with data), SDP (0.5 A with data) |

Because the data lines are cut as well, a device disappears from the bus completely, even a
self-powered one. Switching off is the same as pulling the plug; switching on triggers a
fresh enumeration.

### Switched 5V output

| Item | Value |
|---|---|
| Connector | two pin breakout, 3.24 mm pitch, (+) is +5V, (−) is GND |
| Max continuous current | 6 A |
| On resistance | 10 mΩ |
| Switch | the same type as on the downstream ports |

No USB, power only. The output voltage sags with the load current in proportion to R_ON.

### GPIO

| Item | Value |
|---|---|
| Count | 3 |
| Output levels | high about 3.3 V, low about 0 V |
| Buffers | GPIO 1 and 2 are TTL, GPIO 3 is a Schmitt trigger |
| Input thresholds, TTL | low 0.50 V max, high 1.63 V min |
| Input thresholds, Schmitt | low 0.6 V max, high 2.64 V min |
| Max current per pin | 25 mA, sourcing and sinking |
| Absolute limit per pin | −0.3 V to 3.6 V |

Two modes: as a **control interface**, where the pins become inputs and the like-numbered
downstream port follows the level on the pin, or as a **GPIO/USB bridge**, where the host
uses the pins as digital I/O. `--gpio enable|disable` switches between them, taking effect
after the next reset.

### I2C

| Item | Value |
|---|---|
| Lines | SDA on pin SDI, SCL on pin SCK, pull-ups on the board |
| Factory address of the board | 7 bit `0b1010011` = `0x53` |
| Address format in the protocol | 7 bit address aligned to the top of the byte, LSB zero — the address shifted left by one |
| Slave mode | the board becomes a control interface an outside master can switch |
| Master mode | the board becomes a USB-to-I2C bridge |
| Payload per transfer | 60 bytes at most |
| Factory state | both modes off, and the setting is persistent |
| `--i2c-master enable` | makes the board re-enumerate; unreachable for about a second |
| `--i2c-master disable` | is acknowledged but often does not take — see [Checked against the hardware](#checked-against-the-hardware) |
| Missing ACK from a slave | is **not** reported: a read returns `0xff`, a write reports either success or a transmission error |

Command bytes an outside master sends to the YKUSH3 in **slave mode**:

| Byte | Effect | Byte | Effect |
|---|---|---|---|
| `0x03` | port 1 on | `0x07` | port 1 off |
| `0x04` | port 2 on | `0x08` | port 2 off |
| `0x05` | port 3 on | `0x09` | port 3 off |
| `0x0B` | all ports on | `0x0C` | all ports off |

### Other connectors

Broken out but unused by `ykushcmd`: **UART** (RX/DT, TX/CK) and **SPI** (SS, SDO, SDI,
SCK), plus an **ICSP** header for programming firmware and bootloader. The factory firmware
has no commands for UART or SPI.

### Power and operating conditions

| Item | Value |
|---|---|
| Supply modes | bus powered or self powered, selected by a jumper |
| External supply | 5.00–5.25 V through a screw terminal, 3.54 mm pitch |
| Operating temperature | −40 to +85 °C |
| Absolute limit on the +PWR 5V pin | −0.3 V to +6 V |

Under a heavy load — several hard drives — self powered with an adequate supply is the way
to go; the drop across the power switches grows with the current.

### Hardware revision 1.3.0

A drop-in replacement with no change to features or layout. New: independent switching
sub-circuits per port, an extra GND pin on the GPIO header, better conditioning of the data
lines. Requires firmware 1.5.0 or newer.

## USB control protocol

The host is the master and the board answers. Command and answer are 64 bytes each. Byte 0
of an answer is `0x01` on success and `0x00` on failure.

| Opcode | Command | Layout | Answer |
|---|---|---|---|
| `0x01`–`0x03`, `0x0A` | port 1–3 / all off | `[op]` | `0x01`, echo |
| `0x11`–`0x13`, `0x1A` | port 1–3 / all on | `[op]` | `0x01`, echo |
| `0x21`–`0x23` | port state | `[op]` | byte 1: high nibble on/off, low nibble port number |
| `0x04` / `0x14` / `0x24` | 5V output off / on / state | `[op]` | `0x01`, byte 1 is `0x04`/`0x14` |
| `0x30` | read GPIO | `[op, pin]` | byte 3 is the level |
| `0x31` | write GPIO | `[op, pin, value]` | echo |
| `0x32` | GPIO control interface | `[op, 0/1]` | `0x01, 0x32, action` |
| `0x41` | power-on state | `[op, port, state]` | `0x01, 0x41, port, state` |
| `0x42` | enter bootloader | `[op]` | none, the board restarts |
| `0x51` | I2C configuration | `[op, action, value]` | `0x01` |
| `0x52` | I2C write/read | `[op, action, addr, n, data…]` | `status, 0x52, n, data…` |
| `0x55` | reset | `[op]` | none |
| `0x61` | version | `[op, 0x01\|0x02]` | `0x01, 0x61, major, minor, patch` |

`0x51` actions: `0x01` slave mode, `0x02` master mode, `0x03` set own address.
`0x52` actions: `0x01` write, `0x02` read. Status values for `0x52`: `0x01` success,
`0x02` the board is not in master mode, `0x03` a transmission error on the bus.

The table was derived from the C++ original and checked against the vendor reference. Two
things `ykushcmd` uses are **not** documented anywhere: `-c` with port `e`, the external 5V
output, and the configuration value `2` for the last state. Neither appears in datasheet
v1.2.1 or in the online reference, yet both work — see
[Checked against the hardware](#checked-against-the-hardware).

## How the port is put together

The package builds two targets: the `ykush3` **library**, which holds everything that
touches the board, and the `ykush3cmd` **binary**, a command line front end on top of it.

| File | Target | Contents |
|---|---|---|
| `src/lib.rs` | library | the public surface, everything re-exported at the root |
| `src/device.rs` | library | the `Transport` trait and `Board`, its HID implementation |
| `src/ykush3.rs` | library | the protocol: opcodes, reports, reading the answers |
| `src/fake.rs` | library | `FakeBoard`, the test double for the transport, behind the `fake` feature |
| `src/sanitize.rs` | library | the control character filter for text from outside |
| `src/error.rs` | library | the error type all layers share |
| `src/main.rs` | binary | entry point, running a `Command`, output |
| `src/cli.rs` | binary | argument splitting, value checking, the `Command` enum |
| `src/help.rs` | binary | help text |

The layers are separated so that each one can be exercised without hardware:

```
arguments ──cli::parse──> Invocation{serial, Command} ──execute──> Ykush3<T: Transport>
                                                                        │
                                                        Board (hidapi) ─┴─ FakeBoard (test)
```

`Ykush3<T>` is generic over the transport and `Ykush3::open()` gives the variant backed by a
real HID device. `execute()` writes into an `impl Write`, so the output can be checked too.

### Scripting the board from Rust

A Rust program can depend on the library and skip the command line entirely:

```rust
use ykush3::{Port, Ykush3};

let board = Ykush3::open(None)?;            // or Some("Y3N13808")
board.port_down(Port::Downstream(2))?;
println!("{}", board.port_status(Port::Downstream(2))?);
```

`FakeBoard` ships with the library, so code built on it can be tested the same way this
repository tests itself — against a prepared answer instead of a board. It sits behind the
`fake` feature, because its inspection methods panic on misuse as test helpers should; a
build that does not ask for the double gets a library without test gear:

```toml
[dev-dependencies]
ykush3cmd = { path = "../ykush3-rs", features = ["fake"] }
```

```rust
use ykush3::{fake::FakeBoard, Port, Ykush3};

let board = Ykush3::with_transport(FakeBoard::answering(&[0x01]));
board.port_up(Port::Downstream(1))?;
assert_eq!(board.transport().sent_count(), 1);
```

The library is not on crates.io and makes no stability promise yet; it moves with the
command line application.

## Tests

```
cargo test                                    # 87 tests and a doctest, no
                                              # hardware needed
cargo test -- --ignored --test-threads=1      # 7 more: 5 need a board, 2 only
                                              # the HID stack of the system
```

Every push runs the first line on macOS, along with `cargo fmt --check`, `cargo clippy`
with warnings denied, and a build against the `rust-version` from `Cargo.toml` so that
figure stays honest.

The second run **must** be single threaded. The operating system hands out a HID device
exclusively, and opening or closing it from several threads at once takes the whole process
down with it — SIGTRAP on macOS.

| Group | Tests | Subject |
|---|---|---|
| `cli` | 27 | grammar, board selection, every command, every error message |
| `ykush3` | 31 | the bytes each command sends, reading answers, error paths |
| `main` | 13 | output format, dispatch of every command, program name |
| `error`, `help`, `device`, `sanitize` | 7 | error texts, help output, report padding, the control character filter |
| `tests/cli.rs` | 9 | the built binary: exit codes, stdout versus stderr |
| doctest | 1 | the library example compiles; `no_run`, since it needs a board |
| **running without hardware** | **87 + 1** | |
| `--ignored`, needs a board | 5 | opening with and without a serial number, real exchanges, an acknowledged switch |
| `--ignored`, needs only the HID stack | 2 | enumeration when nothing is attached |

The `FakeBoard` transport records what a command sends and feeds prepared answers back.
That pins the wire bytes of every command against the vendor reference. A `FakeBoard::mute()`
with no answer prepared fails the test as soon as anything reads from it, which is how
`--reset` and `--boot` are held to not waiting for an answer.

### Coverage

Measured with `-C instrument-coverage` across both runs. The figure depends on whether a
board is attached, because the transport in `device.rs` is the one part that cannot be
exercised without one:

| Condition | Lines | When measured |
|---|---|---|
| no board attached | 95.76 % | 2026-08-16, this state, reproducible by anyone |
| board attached | 98.48 % | 2026-08-16, this state, board `Y3N13808` on firmware 1.5.0 |

`cli.rs`, `error.rs`, `fake.rs` and `sanitize.rs` are at 100 % either way; `ykush3.rs`
misses a single line without a board. The 56 lines missing without a board are mostly in
`device.rs`: opening the device, and the transfer and send paths behind it.

With a board attached, what remains unreached is the residue below — plus the untaken half
of the hardware test that re-applies the port state it finds, since only one of its two
branches can run on any given day:

| Place | Why it cannot be reached |
|---|---|
| `device.rs` — `Error::HidInit` | only fires when the operating system's `hid_init()` fails |
| `device.rs` — `Error::NoResponse` | would need a device that returns a zero length report |
| `device.rs` — truncated read, truncated write | would need a device that delivers or takes only part of a report |
| `main.rs` — `unreachable!()` | help, version and listing are handled before a board is opened, so the branch is dead by construction |

Reaching 100 % would take contortions. Turning a documented invariant panic into an error
path that can be triggered artificially does not make the code better. What could be
measured was measured; what could not is written down here.

Reproducing the measurement, with Homebrew LLVM and without `cargo-llvm-cov`:

```
export RUSTFLAGS="-C instrument-coverage" LLVM_PROFILE_FILE="/tmp/yk/%p-%m.profraw"
cargo test && cargo test --no-fail-fast -- --ignored --test-threads=1
llvm-profdata merge -sparse /tmp/yk/*.profraw -o /tmp/yk/yk.profdata
llvm-cov report -object target/debug/deps/<lib-unittest-bin> \
                -object target/debug/deps/<bin-unittest-bin> \
                -object target/debug/deps/<cli-testbin> \
                -object target/debug/ykush3cmd \
                -instr-profile=/tmp/yk/yk.profdata
```

`--no-fail-fast` matters without a board: the library's hardware tests fail first, and
without the flag cargo would stop before the binary's HID stack test has run. The built
binary has to be the last object, otherwise `main()` is missing from the figures.

## Checked against the hardware

Against a YKUSH3 with serial number `Y3N13808`, firmware 1.5.0, bootloader 1.2.0, on macOS
running on Apple silicon. Nothing was attached to the downstream ports.

The answer checking added after the first run of this record was validated against the same
board on 2026-08-16, still on firmware 1.5.0: all seven hardware tests pass, including the
one that pins the ACK the board sets on a switching command.

| Area | What was run | Result |
|---|---|---|
| Test suite | `cargo test -- --ignored --test-threads=1` | 7 hardware tests pass |
| Enumeration | `-l`, `-s <serial>`, an unknown serial | correct, unknown serial exits 1 |
| Versions | firmware, bootloader | 1.5.0 / 1.2.0 |
| Port switching | `-d`/`-u` singly and with `a`, each read back with `-g` | correct |
| 5V output | `-on`/`-off` as well as `-u 4`/`-d 4` | both spellings correct |
| Port state | `-g 1` through `-g 4` | correct, the 5V port included |
| GPIO | `-w`/`-r` on pins 1 and 3, both levels | correct |
| Power-on state | `-c`, verified across real resets | takes effect and persists |
| I2C | master, slave, address, error paths | see below |
| Exit codes | 0 on success, 1 on every error path | correct |

Not exercised: `--boot`. The board would stay in its bootloader and only the firmware update
tool would get it back.

What turned up beyond what the documentation says:

- **The undocumented configuration values work.** `-c e 1` sets the 5V output to come up by
  default, and after a reset it was on. Persistent mode `-c 1 2` holds in both directions:
  a port that was off before the reset stayed off, one that was on stayed on.
- **Changing I2C mode makes the board re-enumerate.** After `--i2c-master enable` it was
  unreachable for about a second. Scripts have to wait, or the commands that follow go
  nowhere.
- **`--i2c-master disable` cannot be relied on.** The board acknowledges the command and
  stays in master mode anyway — after a `--reset` as well, after sending it twice, and after
  an enable/disable cycle. The mode reproducibly went off only after this sequence:
  `--i2c-slave enable` → `--i2c-set-address` → `--i2c-slave disable` →
  `--i2c-master disable` → `--reset`. Which part of it does the work is open; presumably
  cycling slave mode re-initialises the same MSSP unit. Since the setting is persistent,
  only turn master mode on when it is meant to stay on.
- **A missing ACK from an I2C slave is not reported.** On an empty bus `--i2c-read` always
  returns `255` (`0xff`, the idle level of the bus). `--i2c-write` reports success
  sometimes and status `0x03` other times, neither of them because a slave answered. So the
  return value says nothing about whether a device is on the bus. Only the reverse holds:
  status `0x02` definitely means the board is not in master mode.
- **A bus scan across all 7 bit addresses** (`0x08`–`0x77`) ran through without a single
  answer with nothing on the I2C header, so the method is sound for finding a device once
  one is there.
- **The board can only be open once.** A second attempt fails with "exclusive access and
  device already open", so concurrent invocations of `ykush3cmd` have to take turns.
- **The connector is the most common fault.** When the USB 3.0 micro-B plug is not seated
  fully, only the SuperSpeed hub appears; the USB 2.0 hub and the controller are missing and
  the board cannot be reached even though the power LED is lit. Check the plug before
  swapping the cable.

## Where it differs from the C++ original

| Point | Original | Here |
|---|---|---|
| `-r <gpio>` | prints nothing, the value only reaches the exit code | prints the value on stdout, exit code `0` |
| `-g 4` | the answer is not decoded, returns `-1` | the external port is decoded, which tables 6 and 7 of the datasheet say is right |
| `--i2c-read <n>` with n ≥ 10 | `dec2bin()` writes one byte per digit and corrupts the report | the length goes out as a single number |
| hex values without `0x` | are misread | accepted with and without the prefix |
| an invalid port number | help with no explanation | a specific error message |
| `--reset`, `--boot` | wait for an answer the board no longer sends | send without reading |
| `-s` after the command | is ignored | works wherever it appears |
| the external port | `4` for switching, `e` for configuring | both spellings everywhere |
| I2C error status | only `0x00`/`0x01` told apart, otherwise a silent `0` | `0x02` and `0x03` are reported in words |
| messages | — | worded independently, see [License](#license) |

## Driving a YKUR relay board over I2C

The YKUR, Yepkit's USB relay board, has an I2C control interface besides USB and can
therefore be switched by a YKUSH3 in master mode. Both datasheets say so.

**What a YKUR is:** one on-board relay, rated 250 Vac / 30 Vdc at 10 A, plus four driver
outputs for the coils of external relays. Each output handles up to 360 mA at 5 V, but **the
board as a whole is limited to 500 mA**, so four hefty coils at once is not on.

**Addressing:** the YKUR sits at the fixed 7 bit address `0b1010001` = `0x51`. The YKUSH3
wants the address aligned to the top of the byte, so `0x51 << 1 = 0xA2`. No clash with the
YKUSH3's own factory address `0x53`, and it is the master here anyway.

**The command:** a single data byte written to `0xA2`.

| Byte | Effect | Byte | Effect |
|---|---|---|---|
| `0x01` | relay on | `0x02` | relay off |
| `0x03`–`0x06` | port 1–4 on | `0x07`–`0x0A` | port 1–4 off |
| `0x0B` | all ports on | `0x0C` | all ports off |
| `0x0D` | all ports and the relay on | `0x0E` | all ports and the relay off |
| `0x0F` | read the relay state | `0x1F`/`0x2F`/`0x3F`/`0x4F` | read the state of port 1–4 |

Which gives:

```
ykush3cmd --i2c-master enable       # once, and it survives a reset
ykush3cmd --i2c-write 0xA2 0x01     # on-board relay on
ykush3cmd --i2c-write 0xA2 0x02     # on-board relay off
ykush3cmd --i2c-write 0xA2 0x03     # external relay 1 on
ykush3cmd --i2c-write 0xA2 0x0E     # everything off
```

Worth knowing before wiring it up:

- **Measure the levels first.** The YKUSH3 pins take 3.6 V absolute maximum. Whether the
  YKUR pulls its I2C bus to 3.3 V or to 5 V is in neither datasheet. Measure at the YKUR I2C
  pins with the bus idle; at 5 V a level shifter is needed.
- **Both boards carry their own pull-ups.** In parallel that halves the resistance — usually
  harmless, but remove one set if the bus misbehaves.
- **Tie the grounds together.** I2C needs a common reference.
- **The YKUR needs its own supply.** Two I2C wires do not feed it. The obvious arrangement
  is to run the YKUSH3's switched 5V output, good for 6 A, into the YKUR's external supply
  input — which also lets `ykush3cmd -on`/`-off` cut the YKUR's power entirely.
- **The exit code tells you nothing.** Verified on hardware: on an empty bus `--i2c-write`
  reports success and `--i2c-read` returns `255`. Whether the YKUR answers at all shows only
  in the content of a reply that cannot be `255` — or simply in whether the relay clicks.
- **Reading state is untested.** Switching is documented. Whether the query bytes (`0x0F`
  and relatives) work across the bridge — `--i2c-write` with the query byte followed by
  `--i2c-read 0xA2 1` — is in no datasheet, because only the write sequence is described
  there. It has to be tried on the YKUR.
- **Wait after changing mode.** `--i2c-master enable` makes the board re-enumerate and the
  commands that follow fail for about a second. The command itself reports a read error even
  though it was carried out.
- **`enable` is effectively a one-time action.** The mode survives resets and power cuts, and
  turning it off again is awkward — see
  [Checked against the hardware](#checked-against-the-hardware). For running a YKUR that is
  no drawback; the mode is meant to stay on.
- **Firmware:** the YKUR only gained its I2C control interface in revision 1.2.1.

The simpler route, if a PC is standing there anyway: hang the YKUR off a downstream port of
the YKUSH3 over USB and drive it with `ykurcmd`. The I2C route earns its keep when the relay
has to be switched without a second USB device, or from a microcontroller.

## Documents and sources

The vendor documentation is **not** in this repository. It belongs to Yepkit and is not
under the Apache license; redistributing it would be a separate question to ask them. Every
source, with the date it was retrieved, is listed in [`doc/README.md`](doc/README.md).

The main ones:

- [YKUSH3 datasheet](https://www.yepkit.com/uploads/documents/9f39a_ykush3-datasheet.pdf)
- [YKUSH3 USB control interface](https://ykushboards.yepkit.com/docs/ykush3/reference/usb/)
- [YKUSH3 I2C](https://ykushboards.yepkit.com/docs/ykush3/reference/i2c/)
- [YKUR datasheet](https://www.yepkit.com/uploads/documents/bb69a_YKUR_datasheet_Rev.1.2.1.pdf)
- [ykushcmd on GitHub](https://github.com/Yepkit/ykush) — the C++ original this was ported from

## License

Apache License 2.0, full text in [`LICENSE`](LICENSE). Section 5 of the license governs
contributions, see [`CONTRIBUTING.md`](CONTRIBUTING.md).

What was taken from Yepkit's `ykushcmd` is **functional only**: the USB control protocol —
opcodes, report layout, what the answers mean — and the names of the command line switches.
Both are facts and interfaces, and neither is what copyright protects.

No source, no help text and no message of the original is present. Help and output are
written from scratch; that was checked against every string literal in `ykush_help.cpp`,
`ykush3.cpp` and `yk_usb_device.cpp` — nothing overlaps.

The choice of the Apache license is therefore freely made rather than inherited. The credit
in [`NOTICE`](NOTICE) is there voluntarily, because the protocol would not exist without
Yepkit's work. Yepkit has neither endorsed nor reviewed this and does not support it.

HIDAPI, which is linked in statically, is used under its **BSD license** rather than the GPL
it also offers — text in [`LICENSE-hidapi-bsd.txt`](LICENSE-hidapi-bsd.txt). That file has
to accompany a binary release. The remaining components are listed in
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
