//! ttyp — a minimal monkeytype-style typing test for the terminal.
//!
//! The crate is split into UI-free core modules (`test`, `language`, `theme`,
//! `config`, `stats`, `command`, `profile`) and a thin `app`/`ui` layer that renders them.

pub mod app;
pub mod command;
pub mod config;
pub mod gfx;
pub mod language;
pub mod profile;
pub mod stats;
pub mod test;
pub mod theme;
pub mod ui;
