//! ttyp-core — the UI-free heart of ttyp, shared by the terminal client and
//! the server: the typing-test engine, word generation, result metrics and
//! the language registry. No ratatui or crossterm here.

pub mod api;
pub mod language;
pub mod test;
