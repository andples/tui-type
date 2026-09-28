//! Leaderboard screen: the first-try and best boards of one daily, side by
//! side when the terminal is wide enough, otherwise one at a time.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{GUTTER, Palette, content_column};
use super::widgets::{Cell, Column, Panes, Row, SelectTable, Width, hints, panes};
use crate::app::App;
use crate::online::BoardPane;
use ttyp_core::api::{Board, LeaderboardRow};

/// Narrowest a board may be before the two go behind tabs.
const MIN_PANE: u16 = 46;
const PANE_GAP: u16 = 4;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    // Wider than other screens: two tables need the room.
    let col = content_column(area, area.width.saturating_sub(GUTTER * 2));
    let Some(view) = &app.board else {
        return;
    };

    // Title row: what's shown, and how to change it.
    let title = Line::from(vec![
        Span::styled("leaderboard", p.main_bold()),
        Span::styled(
            format!(
                "  {} · {} · {}",
                view.language,
                view.mode.label(),
                view.date
            ),
            p.fg(),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(title),
        Rect::new(col.x, col.y + 2, col.width, 1),
    );
    let switches = "←→ mode · l language · [ ] day";
    let sw = switches.chars().count() as u16;
    if col.width > sw + 40 {
        frame.render_widget(
            Paragraph::new(switches).style(p.sub()),
            Rect::new(col.right().saturating_sub(sw), col.y + 2, sw, 1),
        );
    }

    let body = Rect::new(col.x, col.y + 4, col.width, col.height.saturating_sub(7));
    if let Some(e) = &view.error {
        frame.render_widget(Paragraph::new(e.as_str()).style(p.error()), body);
    } else if view.dailies_loading {
        frame.render_widget(Paragraph::new("loading…").style(p.sub()), body);
    } else if view.daily().is_none() {
        frame.render_widget(Paragraph::new("no daily for this day").style(p.sub()), body);
    } else {
        match panes::split(body, 2, MIN_PANE, PANE_GAP) {
            Panes::SideBySide(rects) => {
                for (board, rect) in [Board::First, Board::Best].into_iter().zip(rects) {
                    let focused = view.focus == board;
                    render_pane(frame, view.pane(board), Some(board), focused, rect, p);
                }
            }
            Panes::Tabs(rect) => {
                let tabs: Vec<Span> = [Board::First, Board::Best]
                    .into_iter()
                    .flat_map(|b| {
                        let style = if b == view.focus {
                            p.selected()
                        } else {
                            p.sub()
                        };
                        [Span::styled(b.label(), style), Span::raw("    ")]
                    })
                    .collect();
                frame.render_widget(
                    Paragraph::new(Line::from(tabs)),
                    Rect::new(rect.x, rect.y, rect.width, 1),
                );
                let table = Rect::new(
                    rect.x,
                    rect.y + 2,
                    rect.width,
                    rect.height.saturating_sub(2),
                );
                render_pane(frame, view.pane(view.focus), None, true, table, p);
            }
        }
    }

    let hint = match panes::split(body, 2, MIN_PANE, PANE_GAP) {
        Panes::SideBySide(_) => "enter graph · ↑↓ move · tab switch board · esc back",
        Panes::Tabs(_) => "enter graph · ↑↓ move · tab other board · esc back",
    };
    hints::render(frame, area, col, p, hint);
}

fn columns() -> [Column; 5] {
    [
        Column::new("#", Width::Fixed(3)).right(),
        Column::new("name", Width::Min(8)).highlight(),
        Column::new("wpm", Width::Fixed(5)).right(),
        Column::new("acc", Width::Fixed(6)).right(),
        Column::new("con", Width::Fixed(4)).right(),
    ]
}

fn row(r: &LeaderboardRow, me: bool) -> Row {
    let row = Row::new(vec![
        Cell::dim(r.rank.to_string()),
        Cell::normal(if me { "you" } else { r.user.as_str() }),
        Cell::normal(format!("{:.0}", r.wpm)),
        Cell::dim(format!("{:.1}%", r.acc)),
        Cell::dim(format!("{:.0}%", r.consistency)),
    ]);
    if me { row.accent() } else { row }
}

/// One board: its title (when side by side; tabs already name it), then
/// the table, or why there isn't one.
fn render_pane(
    frame: &mut Frame,
    pane: &BoardPane,
    title: Option<Board>,
    focused: bool,
    area: Rect,
    p: &Palette,
) {
    let mut table_area = area;
    if let Some(board) = title {
        let style = if focused { p.main_bold() } else { p.sub() };
        frame.render_widget(
            Paragraph::new(board.label()).style(style),
            Rect::new(area.x, area.y, area.width, 1),
        );
        table_area = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
    }
    let status = if let Some(e) = &pane.error {
        Some(Span::styled(e.as_str(), p.error()))
    } else if pane.rows.is_empty() && pane.loading {
        Some(Span::styled("loading…", p.sub()))
    } else if pane.rows.is_empty() {
        Some(Span::styled("no results yet", p.sub()))
    } else {
        None
    };
    if let Some(s) = status {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::raw("  "), s])),
            Rect::new(table_area.x, table_area.y + 1, table_area.width, 1),
        );
        return;
    }

    let my_id = pane.me.as_ref().map(|m| m.result_id);
    let rows: Vec<Row> = pane
        .rows
        .iter()
        .map(|r| row(r, Some(r.result_id) == my_id))
        .collect();
    let columns = columns();
    let mut table = SelectTable::new(&columns, rows, &pane.selection).focused(focused);
    if let Some(me) = &pane.me {
        // Off-screen (or not fetched yet): pin at the bottom.
        let index = pane.my_index().unwrap_or(usize::MAX);
        table = table.pinned(index, row(me, true));
    }
    table.render(frame, table_area, p);
}
