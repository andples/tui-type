//! Small building blocks for list-style screens: a cursor with scrolling
//! (`Selection`), a themed table that renders it (`SelectTable`), a way to
//! put several tables side by side when the terminal is wide enough
//! (`panes`) and the bottom hint line every screen has (`hints`).

pub mod hints;
pub mod panes;
pub mod selection;
pub mod table;

pub use panes::Panes;
pub use selection::Selection;
pub use table::{Align, Cell, Column, Role, Row, SelectTable, Width};
