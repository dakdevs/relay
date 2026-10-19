//! Relay desktop: GPUI tray popover and main window over the local proxy.
//! The proxy runs on its own runtime; this crate only renders snapshots.

pub mod accounts;
pub mod adapter;
pub mod app;
pub mod assets;
pub mod bridge;
pub mod chart;
pub mod cli;
pub mod dashboard;
pub mod format;
pub mod input;
pub mod login;
pub mod model;
pub mod motion;
pub mod popover;
pub mod quotas;
pub mod routing;
pub mod store;
pub mod theme;
pub mod through;
pub mod tray;
pub mod usage;
pub mod widgets;

/// Display version `YYYYMMDD.N`.
pub const DISPLAY_VERSION: &str = relay_core::DISPLAY_VERSION;
