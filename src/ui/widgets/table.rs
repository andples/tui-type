//! A themed table with a cursor: thin layer over ratatui's `Table` that adds
//! column specs, cell style roles, `…` truncation, the shared selected-row
//! look (`cursor`) and a row pinned to the bottom when it scrolls out of view.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Span, Text};
use ratatui::widgets::{HighlightSpacing, Row as TableRow, Table, TableState};

use super::{Selection, cursor};
use crate::ui::style::Palette;

/// Columns of the marker gutter in front of every row.
const MARKER_WIDTH: u16 = cursor::GUTTER;
const SPACING: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Fixed(u16),
    /// At least this wide; shares whatever is left with other `Min` columns.
    Min(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub header: String,
    pub width: Width,
    pub align: Align,
}

impl Column {
    pub fn new(header: impl Into<String>, width: Width) -> Self {
        Self {
            header: header.into(),
            width,
            align: Align::Left,
        }
    }

    pub fn right(mut self) -> Self {
        self.align = Align::Right;
        self
    }
}

/// How a cell is coloured; the table maps roles to the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    Normal,
    Dim,
    Accent,
    Error,
    /// A fixed colour that ignores the palette (theme swatches).
    Swatch(Color),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub role: Role,
    /// Columns this cell covers (a one-cell row spans them all).
    pub span: u16,
}

impl Cell {
    pub fn new(text: impl Into<String>, role: Role) -> Self {
        Self {
            text: text.into(),
            role,
            span: 1,
        }
    }

    pub fn normal(text: impl Into<String>) -> Self {
        Self::new(text, Role::Normal)
    }

    pub fn dim(text: impl Into<String>) -> Self {
        Self::new(text, Role::Dim)
    }

    pub fn accent(text: impl Into<String>) -> Self {
        Self::new(text, Role::Accent)
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self::new(text, Role::Error)
    }

    pub fn swatch(text: impl Into<String>, color: Color) -> Self {
        Self::new(text, Role::Swatch(color))
    }

    pub fn span(mut self, columns: u16) -> Self {
        self.span = columns.max(1);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub cells: Vec<Cell>,
    /// The row that is "you": its `Normal` cells take the accent colour.
    pub accent: bool,
}

impl Row {
    pub fn new(cells: Vec<Cell>) -> Self {
        Self {
            cells,
            accent: false,
        }
    }

    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }
}

pub struct SelectTable<'a> {
    columns: &'a [Column],
    rows: Vec<Row>,
    selection: &'a Selection,
    focused: bool,
    /// A row to keep on screen: its index in `rows` and how to draw it.
    pinned: Option<(usize, Row)>,
}

impl<'a> SelectTable<'a> {
    pub fn new(columns: &'a [Column], rows: Vec<Row>, selection: &'a Selection) -> Self {
        Self {
            columns,
            rows,
            selection,
            focused: true,
            pinned: None,
        }
    }

    /// An unfocused table keeps its cursor but shows no marker.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn pinned(mut self, index: usize, row: Row) -> Self {
        self.pinned = Some((index, row));
        self
    }

    /// Rows of `area` a table with these columns needs to show `n` rows.
    pub fn height_for(columns: &[Column], n: usize) -> u16 {
        n as u16 + u16::from(has_header(columns))
    }

    pub fn render(self, frame: &mut Frame, area: Rect, p: &Palette) {
        let header = has_header(self.columns);
        let widths = resolve_widths(self.columns, area.width);
        let mut body = area.height.saturating_sub(u16::from(header)) as usize;
        self.selection.ensure_visible(body);

        // The pinned row takes the last line only when it isn't in view.
        let pinned = self.pinned.filter(|(i, _)| !self.selection.is_visible(*i));
        if pinned.is_some() && body > 1 {
            body -= 1;
            self.selection.ensure_visible(body);
        }
        let list_area = Rect::new(
            area.x,
            area.y,
            area.width,
            area.height - u16::from(pinned.is_some()),
        );

        let selected = self.focused.then_some(self.selection.selected);
        let rows = self
            .rows
            .into_iter()
            .enumerate()
            .map(|(i, r)| table_row(r, self.columns, &widths, p, selected == Some(i)));
        let mut table = Table::new(rows, constraints(&widths))
            .column_spacing(SPACING)
            // The row's own left-aligned cells add the third column of
            // ` › `, so the selected row moves right (`cursor::lead`).
            .highlight_symbol(Span::styled(" ›", p.main()))
            .highlight_spacing(HighlightSpacing::Always);
        if header {
            let cells = self.columns.iter().enumerate().map(|(i, c)| {
                let mut text = truncate(&c.header, widths[i].saturating_sub(NUDGE) as usize);
                if c.align == Align::Right {
                    text.push(' ');
                }
                aligned(text, c.align)
            });
            table = table.header(TableRow::new(cells).style(p.sub()));
        }
        let mut state = TableState::new()
            .with_offset(self.selection.offset())
            .with_selected(selected);
        frame.render_stateful_widget(table, list_area, &mut state);

        if let Some((_, row)) = pinned {
            let last = Rect::new(area.x, area.bottom() - 1, area.width, 1);
            // Same blank marker gutter as the list, so the columns line up.
            let table = Table::new(
                [table_row(row, self.columns, &widths, p, false)],
                constraints(&widths),
            )
            .column_spacing(SPACING)
            .highlight_symbol("  ")
            .highlight_spacing(HighlightSpacing::Always);
            frame.render_stateful_widget(table, last, &mut TableState::new());
        }
    }
}

fn has_header(columns: &[Column]) -> bool {
    columns.iter().any(|c| !c.header.is_empty())
}

fn constraints(widths: &[u16]) -> Vec<Constraint> {
    widths.iter().map(|w| Constraint::Length(*w)).collect()
}

/// Spare column every column keeps for the selected row's nudge.
const NUDGE: u16 = 1;

/// Column widths for `total` columns: fixed ones as asked, the rest shared
/// equally by the `Min` columns (each at least its minimum). Each gets one
/// more for the nudge, so a selected row moves whole and isn't cut short.
fn resolve_widths(columns: &[Column], total: u16) -> Vec<u16> {
    let spacing = SPACING * columns.len().saturating_sub(1) as u16;
    let nudges = NUDGE * columns.len() as u16;
    let mut free = total.saturating_sub(MARKER_WIDTH + spacing + nudges);
    let mut flexible = 0u16;
    for c in columns {
        match c.width {
            Width::Fixed(w) => free = free.saturating_sub(w),
            Width::Min(w) => {
                free = free.saturating_sub(w);
                flexible += 1;
            }
        }
    }
    let share = free.checked_div(flexible).unwrap_or(0);
    let mut extra = free.checked_rem(flexible).unwrap_or(0);
    columns
        .iter()
        .map(|c| {
            NUDGE
                + match c.width {
                    Width::Fixed(w) => w,
                    Width::Min(w) => {
                        let bonus = u16::from(extra > 0);
                        extra = extra.saturating_sub(1);
                        w + share + bonus
                    }
                }
        })
        .collect()
}

fn table_row<'b>(
    row: Row,
    columns: &[Column],
    widths: &[u16],
    p: &Palette,
    selected: bool,
) -> TableRow<'b> {
    let mut col = 0usize;
    let cells = row.cells.into_iter().map(|cell| {
        let span = (cell.span as usize)
            .min(columns.len().saturating_sub(col))
            .max(1);
        // A spanning cell also gets the spacing between the columns it covers.
        let width: u16 = widths[col..col + span].iter().sum::<u16>() + SPACING * (span as u16 - 1);
        let column = &columns[col.min(columns.len().saturating_sub(1))];
        let style = cell_style(cell.role, row.accent, selected, p);
        col += span;
        // The spare column sits before the text on the selected row and
        // after it otherwise, so the whole row moves right by one.
        let text = truncate(&cell.text, width.saturating_sub(NUDGE) as usize);
        let text = match (selected, column.align) {
            (true, Align::Left) => format!(" {text}"),
            (false, Align::Right) => format!("{text} "),
            _ => text,
        };
        ratatui::widgets::Cell::new(aligned(text, column.align))
            .style(style)
            .column_span(span as u16)
    });
    TableRow::new(cells.collect::<Vec<_>>())
}

fn cell_style(role: Role, accent_row: bool, highlight: bool, p: &Palette) -> Style {
    match role {
        Role::Normal | Role::Dim | Role::Accent if highlight => p.selected(),
        Role::Normal if accent_row => p.main(),
        Role::Normal => p.fg(),
        Role::Dim => p.sub(),
        Role::Accent => p.main(),
        Role::Error => p.error(),
        Role::Swatch(c) => Style::default().fg(c),
    }
}

fn aligned(text: String, align: Align) -> Text<'static> {
    let t = Text::from(text);
    match align {
        Align::Left => t,
        Align::Right => t.alignment(Alignment::Right),
    }
}

/// `s` cut to `width` columns with a trailing `…` when it didn't fit.
pub fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_share_the_rest() {
        let cols = [
            Column::new("", Width::Fixed(1)),
            Column::new("", Width::Min(4)),
            Column::new("", Width::Min(0)),
        ];
        // 40 − marker 2 − spacing 2 − nudges 3 − fixed 1 − mins 4 = 28 to
        // share; each column keeps its spare nudge column.
        assert_eq!(resolve_widths(&cols, 40), vec![2, 19, 15]);
        assert_eq!(resolve_widths(&cols, 3), vec![2, 5, 1]);
    }

    #[test]
    fn truncates_with_ellipsis() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello world", 5), "hell…");
        assert_eq!(truncate("héllo", 3), "hé…");
        assert_eq!(truncate("héllo", 0), "…");
    }
}
