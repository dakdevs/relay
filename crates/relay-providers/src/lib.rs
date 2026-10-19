//! Wire-format knowledge for the two supported APIs, as pure functions over
//! `http` and byte types. Nothing here does I/O.
#![warn(missing_docs)]

pub mod auth;
pub mod headers;
pub mod quota;
pub mod request;
pub mod sse;
pub mod tap;
