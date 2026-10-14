//! A themed table with a cursor: thin layer over ratatui's `Table` that adds
//! column specs, cell style roles, `…` truncation, the shared selected-row
//! look (`cursor`) and a row pinned to the bottom when it scrolls out of view.
//! Columns grow to fit their content when there's room; whatever still
//! doesn't fit is cut with `…`, and the selected row wraps to show it whole.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
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
        let widths = resolve_widths(
            self.columns,
            &natural_widths(self.columns, &self.rows),
            area.width,
        );
        let mut body = area.height.saturating_sub(u16::from(header)) as usize;
        let selected = self.focused.then_some(self.selection.selected);
        // The selected row wraps what was cut, so it may take several lines;
        // the window holds that many fewer rows.
        let tall = selected
            .and_then(|i| self.rows.get(i))
            .map_or(1, |r| row_height(r, self.columns, &widths).min(body.max(1)));
        body = body.saturating_sub(tall - 1).max(1);
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

/// Widest header or single-column cell in each column.
fn natural_widths(columns: &[Column], rows: &[Row]) -> Vec<u16> {
    let mut widths: Vec<u16> = columns.iter().map(|c| text_width(&c.header)).collect();
    for row in rows {
        let mut col = 0usize;
        for cell in &row.cells {
            if cell.span <= 1
                && let Some(w) = widths.get_mut(col)
            {
                *w = (*w).max(text_width(&cell.text));
            }
            col += cell.span.max(1) as usize;
        }
    }
    widths
}

fn text_width(s: &str) -> u16 {
    s.chars().count().min(u16::MAX as usize) as u16
}

/// Column widths for `total` columns. Each starts at its spec (fixed ones as
/// asked, `Min` ones at their minimum); the room left first grows columns
/// whose `natural` content is wider, one column at a time round the table so
/// none is starved, and the rest is shared equally by the `Min` columns.
/// Each gets one more for the nudge, so a selected row moves whole and isn't
/// cut short.
fn resolve_widths(columns: &[Column], natural: &[u16], total: u16) -> Vec<u16> {
    let spacing = SPACING * columns.len().saturating_sub(1) as u16;
    let nudges = NUDGE * columns.len() as u16;
    let mut free = total.saturating_sub(MARKER_WIDTH + spacing + nudges);
    let mut widths: Vec<u16> = columns
        .iter()
        .map(|c| match c.width {
            Width::Fixed(w) | Width::Min(w) => w,
        })
        .collect();
    free = free.saturating_sub(widths.iter().sum());

    let want = |widths: &[u16], i: usize| natural.get(i).copied().unwrap_or(0) > widths[i];
    while free > 0 && (0..widths.len()).any(|i| want(&widths, i)) {
        for i in 0..widths.len() {
            if free > 0 && want(&widths, i) {
                widths[i] += 1;
                free -= 1;
            }
        }
    }

    let flexible = columns
        .iter()
        .filter(|c| matches!(c.width, Width::Min(_)))
        .count() as u16;
    let share = free.checked_div(flexible).unwrap_or(0);
    let mut extra = free.checked_rem(flexible).unwrap_or(0);
    columns
        .iter()
        .zip(widths)
        .map(|(c, w)| {
            NUDGE
                + match c.width {
                    Width::Fixed(_) => w,
                    Width::Min(_) => {
                        let bonus = u16::from(extra > 0);
                        extra = extra.saturating_sub(1);
                        w + share + bonus
                    }
                }
        })
        .collect()
}

/// Each cell's width (without the nudge) and column, spans included.
fn cell_widths<'r>(
    row: &'r Row,
    columns: &[Column],
    widths: &[u16],
) -> Vec<(&'r Cell, usize, usize)> {
    let mut out = Vec::new();
    let mut col = 0usize;
    for cell in &row.cells {
        if col >= columns.len() {
            break;
        }
        let span = (cell.span as usize).min(columns.len() - col).max(1);
        // A spanning cell also gets the spacing between the columns it covers.
        let width: u16 = widths[col..col + span].iter().sum::<u16>() + SPACING * (span as u16 - 1);
        out.push((cell, col, width.saturating_sub(NUDGE) as usize));
        col += span;
    }
    out
}

/// Lines the row needs when selected: its tallest wrapped cell.
fn row_height(row: &Row, columns: &[Column], widths: &[u16]) -> usize {
    cell_widths(row, columns, widths)
        .into_iter()
        .map(|(cell, _, width)| wrap(&cell.text, width).len())
        .max()
        .unwrap_or(1)
        .max(1)
}

fn table_row<'b>(
    row: Row,
    columns: &[Column],
    widths: &[u16],
    p: &Palette,
    selected: bool,
) -> TableRow<'b> {
    let mut height = 1;
    let cells: Vec<_> = cell_widths(&row, columns, widths)
        .into_iter()
        .map(|(cell, col, width)| {
            let column = &columns[col];
            let style = cell_style(cell.role, row.accent, selected, p);
            // The selected row shows the whole text, wrapped; others cut it.
            let lines = if selected {
                wrap(&cell.text, width)
            } else {
                vec![truncate(&cell.text, width)]
            };
            height = height.max(lines.len());
            // The spare column sits before the text on the selected row and
            // after it otherwise, so the whole row moves right by one.
            let lines: Vec<Line> = lines
                .into_iter()
                .map(|text| match (selected, column.align) {
                    (true, Align::Left) => format!(" {text}"),
                    (false, Align::Right) => format!("{text} "),
                    _ => text,
                })
                .map(Line::from)
                .collect();
            let span = cell.span.max(1).min((columns.len() - col) as u16);
            ratatui::widgets::Cell::new(aligned(Text::from(lines), column.align))
                .style(style)
                .column_span(span)
        })
        .collect();
    TableRow::new(cells).height(height as u16)
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

fn aligned(text: impl Into<Text<'static>>, align: Align) -> Text<'static> {
    let t = text.into();
    match align {
        Align::Left => t,
        Align::Right => t.alignment(Alignment::Right),
    }
}

/// `s` in lines of at most `width` columns, broken at spaces where it can
/// be and mid-word where it can't. Always at least one line.
pub fn wrap(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split(' ') {
        let len = line.chars().count();
        let wlen = word.chars().count();
        if len > 0 && len + 1 + wlen <= width {
            line.push(' ');
            line.push_str(word);
            continue;
        }
        if len > 0 {
            lines.push(std::mem::take(&mut line));
        }
        let mut chars = word.chars().peekable();
        while chars.peek().is_some() {
            let chunk: String = chars.by_ref().take(width).collect();
            if chars.peek().is_some() {
                lines.push(chunk);
            } else {
                line = chunk;
            }
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
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
        assert_eq!(resolve_widths(&cols, &[0; 3], 40), vec![2, 19, 15]);
        assert_eq!(resolve_widths(&cols, &[0; 3], 3), vec![2, 5, 1]);
    }

    #[test]
    fn wide_content_grows_its_column_first() {
        // The profile's bests: "words 100" doesn't fit a mode column of 8.
        let cols = [
            Column::new("language", Width::Min(8)),
            Column::new("mode", Width::Fixed(8)),
            Column::new("wpm", Width::Fixed(4)),
        ];
        // 40 − 2 − 2 − 3 − 20 = 13 free: mode takes 1, language the rest.
        assert_eq!(resolve_widths(&cols, &[7, 9, 3], 40), vec![21, 10, 5],);
        // 29 leaves 2 free, not enough for both: the growth goes round the table.
        assert_eq!(resolve_widths(&cols, &[12, 12, 3], 29), vec![10, 10, 5],);
    }

    #[test]
    fn natural_widths_skip_spanning_cells() {
        let cols = [
            Column::new("a", Width::Min(0)),
            Column::new("bb", Width::Min(0)),
        ];
        let rows = [
            Row::new(vec![Cell::normal("xyz"), Cell::normal("q")]),
            Row::new(vec![Cell::normal("a much longer message").span(2)]),
        ];
        assert_eq!(natural_widths(&cols, &rows), vec![3, 2]);
    }

    #[test]
    fn wraps_at_spaces_then_mid_word() {
        assert_eq!(wrap("words 100", 8), ["words", "100"]);
        assert_eq!(wrap("time 15", 8), ["time 15"]);
        assert_eq!(wrap("code_python+numpy", 8), ["code_pyt", "hon+nump", "y"]);
        assert_eq!(wrap("ab cdefghij", 4), ["ab", "cdef", "ghij"]);
        assert_eq!(wrap("", 4), [""]);
    }

    #[test]
    fn truncates_with_ellipsis() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello world", 5), "hell…");
        assert_eq!(truncate("héllo", 3), "hé…");
        assert_eq!(truncate("héllo", 0), "…");
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn draw(focused: bool, selected: usize, width: u16) -> String {
        let p =
            Palette::from_theme(crate::theme::ThemeRegistry::builtin().get_or_default("default"));
        let columns = [
            Column::new("language", Width::Min(8)),
            Column::new("mode", Width::Fixed(8)),
            Column::new("wpm", Width::Fixed(4)).right(),
            Column::new("acc", Width::Fixed(4)).right(),
            Column::new("date", Width::Fixed(10)),
        ];
        let data = [
            ("english", "words 10", "145"),
            ("code_python+numpy", "words 100", "110"),
            ("english", "time 15", "135"),
        ];
        let rows = data
            .iter()
            .map(|(l, m, w)| {
                Row::new(vec![
                    Cell::normal(*l),
                    Cell::dim(*m),
                    Cell::normal(*w),
                    Cell::dim("97%"),
                    Cell::dim("2026-10-05"),
                ])
            })
            .collect();
        let mut sel = Selection::clamped(data.len());
        sel.select(selected);
        let mut term = Terminal::new(TestBackend::new(width, 6)).unwrap();
        term.draw(|f| {
            SelectTable::new(&columns, rows, &sel)
                .focused(focused)
                .render(f, f.area(), &p)
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                let line: String = (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn wide_content_fits_when_there_is_room() {
        let out = draw(false, 0, 60);
        assert!(
            out.contains("code_python+numpy       words 100   110"),
            "{out}"
        );
    }

    #[test]
    fn selected_row_wraps_what_was_cut() {
        let out = draw(true, 1, 42);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[2].starts_with(" › code_pyt  words"), "{out}");
        assert!(lines[3].contains("hon+nump  100"), "{out}");
        assert!(lines[4].trim() == "y", "{out}");
        assert!(lines[5].starts_with("  english   time 15"), "{out}");
        // Unselected rows are still cut.
        assert!(draw(true, 0, 42).contains("code_py…  words 1…"));
    }
}
