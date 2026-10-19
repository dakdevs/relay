//! Usage recording and queries.
//!
//! The request path calls [`Sink::record`], which never blocks: it uses a
//! bounded channel and counts drops. A dedicated thread batches events into
//! SQLite (WAL). [`Reader`] answers UI queries on its own read-only
//! connection.
#![warn(missing_docs)]

pub mod reader;
pub mod schema;
pub mod writer;

pub use reader::{AccountTotals, Range, Reader};
pub use writer::{Recorder, Sink, UsageEvent};
