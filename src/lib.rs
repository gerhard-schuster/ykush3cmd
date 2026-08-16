// SPDX-License-Identifier: Apache-2.0
//! Control library for the Yepkit YKUSH3 switchable USB hub.
//!
//! Rust port of the YKUSH3 part of the Yepkit `ykushcmd` application. The
//! `ykush3cmd` binary in this package is a command line front end to this
//! library; a Rust program can skip it and drive the board directly:
//!
//! ```no_run
//! use ykush3::{Port, Ykush3};
//!
//! # fn main() -> ykush3::Result<()> {
//! let board = Ykush3::open(None)?;
//! board.port_down(Port::Downstream(2))?;
//! println!("{}", board.port_status(Port::Downstream(2))?);
//! # Ok(())
//! # }
//! ```
//!
//! Code built on this library can be tested without hardware: behind the
//! `fake` feature, `fake::FakeBoard` stands in for a board, answers from a
//! prepared queue and records what would have been sent. The feature is off
//! by default, so the plain library carries no test gear.

// This library is written for macOS on Apple silicon, and has only ever been
// built and run there. Anywhere else it refuses to compile rather than produce
// a binary nobody has tried.
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
compile_error!("ykush3 is for macOS on Apple silicon");

mod device;
mod error;
mod sanitize;
mod ykush3;

#[cfg(any(test, feature = "fake"))]
pub mod fake;

pub use crate::device::{list, Board, Report, Transport, PRODUCT_ID, REPORT_SIZE, VENDOR_ID};
pub use crate::error::{Error, Result};
pub use crate::sanitize::sanitize;
pub use crate::ykush3::{Port, PortStatus, PowerOnState, Version, Ykush3, I2C_MAX_BYTES};
