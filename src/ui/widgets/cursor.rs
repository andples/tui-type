//! The one look for "this item is selected", shared by every menu and
//! table: a `›` marker, the item nudged one column right and drawn in the
//! selection colour. Unselected items sit at the same column without it.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::ui::style::Palette;

/// Columns in front of an unselected item.
pub const GUTTER: u16 = 2;
/// Columns in front of a selected one: one more, so it moves right.
pub const SELECTED_GUTTER: u16 = GUTTER + 1;

/// What goes in front of an item: ` › ` when selected, two blanks if not.
pub fn lead(p: &Palette, selected: bool) -> Span<'static> {
    if selected {
        Span::styled(" › ", p.main())
    } else {
        Span::raw("  ")
    }
}

/// The selection style for a selected item's text, `normal` otherwise.
pub fn style(p: &Palette, selected: bool, normal: Style) -> Style {
    if selected { p.selected() } else { normal }
}
