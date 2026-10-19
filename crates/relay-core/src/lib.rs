//! Relay domain model shared by the proxy, the usage store and the desktop app.
//!
//! Everything here is pure: no sockets, files or async runtime. Times are Unix
//! milliseconds (`i64`) supplied by the caller so behaviour is deterministic in
//! tests.
#![warn(missing_docs)]

pub mod aggregate;
pub mod attempt;
pub mod config;
pub mod model;
pub mod period;
pub mod quota;
pub mod select;
pub mod snapshot;
pub mod usage;

/// Unix time in milliseconds.
pub type UnixMs = i64;

/// Display version, `YYYYMMDD.N`, derived from the Cargo version `YYYYMMDD.N.0`.
pub const DISPLAY_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
);
