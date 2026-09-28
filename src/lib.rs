//! ttyp — a minimal monkeytype-style typing test for the terminal.
//!
//! The typing-test engine and languages live in `ttyp-core` (shared with the
//! server) and are re-exported here. The rest is UI-free modules (`theme`,
//! `config`, `stats`, `command`, `profile`) and a thin `app`/`ui` layer that
//! renders them.

pub mod app;
pub mod command;
pub mod config;
pub mod gfx;
pub mod online;
pub mod profile;
pub mod stats;
pub mod theme;
pub mod ui;

pub use ttyp_core::{language, test};
