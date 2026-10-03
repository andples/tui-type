//! Leaderboard screen: the boards of one period (`boards::on`), side by
//! side when the terminal is wide enough, otherwise one at a time. Titles
//! and columns come from each board's `BoardSpec`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{GUTTER, Palette, content_column};
use super::widgets::{Cell, Column, Panes, Row, SelectTable, Width, hints, panes};
use crate::app::App;
use crate::online::BoardPane;
use ttyp_core::api::LeaderboardRow;
use ttyp_core::boards::{self, BoardSpec, Period, Stat};

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
    let when = match view.period {
        Period::Daily => view.date.as_str(),
        Period::AllTime => view.period.label(),
    };
    let title = Line::from(vec![
        Span::styled("leaderboard", p.main_bold()),
        Span::styled(
            format!("  {} · {} · {when}", view.language, view.mode.label()),
            p.fg(),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(title),
        Rect::new(col.x, col.y + 2, col.width, 1),
    );
    let periods = boards::periods();
    let next = periods
        .iter()
        .cycle()
        .skip_while(|p| **p != view.period)
        .nth(1)
        .filter(|p| **p != view.period);
    let mut switches = String::from("←→ mode · l language");
    if view.period == Period::Daily {
        switches.push_str(" · [ ] day");
    }
    if let Some(next) = next {
        switches.push_str(&format!(" · a {}", next.label()));
    }
    let sw = switches.chars().count() as u16;
    if col.width > sw + 40 {
        frame.render_widget(
            Paragraph::new(switches.as_str()).style(p.sub()),
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
        let specs = view.boards();
        match panes::split(body, specs.len(), MIN_PANE, PANE_GAP) {
            Panes::SideBySide(rects) => {
                for (i, (spec, rect)) in specs.iter().zip(rects).enumerate() {
                    let focused = view.focus == i;
                    render_pane(frame, &view.panes[i], spec, true, focused, rect, p);
                }
            }
            Panes::Tabs(rect) => {
                let tabs: Vec<Span> = specs
                    .iter()
                    .enumerate()
                    .flat_map(|(i, b)| {
                        let style = if i == view.focus {
                            p.selected()
                        } else {
                            p.sub()
                        };
                        [Span::styled(b.label, style), Span::raw("    ")]
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
                let spec = specs[view.focus];
                render_pane(frame, &view.panes[view.focus], spec, false, true, table, p);
            }
        }
    }

    let hint = match panes::split(body, view.panes.len(), MIN_PANE, PANE_GAP) {
        Panes::SideBySide(_) => "enter graph · p profile · ↑↓ move · tab switch board · esc back",
        Panes::Tabs(_) => "enter graph · p profile · ↑↓ move · tab other board · esc back",
    };
    hints::render(frame, area, col, p, hint);
}

fn column(stat: Stat) -> Column {
    match stat {
        Stat::Rank => Column::new("#", Width::Fixed(3)).right(),
        Stat::User => Column::new("name", Width::Min(8)),
        Stat::Wpm => Column::new("wpm", Width::Fixed(5)).right(),
        Stat::Acc => Column::new("acc", Width::Fixed(6)).right(),
        Stat::Consistency => Column::new("con", Width::Fixed(4)).right(),
        Stat::Date => Column::new("date", Width::Fixed(10)).right(),
    }
}

fn cell(stat: Stat, r: &LeaderboardRow, me: bool) -> Cell {
    match stat {
        Stat::Rank => Cell::dim(r.rank.to_string()),
        Stat::User => Cell::normal(if me { "you" } else { r.user.as_str() }),
        Stat::Wpm => Cell::normal(format!("{:.0}", r.wpm)),
        Stat::Acc => Cell::dim(format!("{:.1}%", r.acc)),
        Stat::Consistency => Cell::dim(format!("{:.0}%", r.consistency)),
        Stat::Date => Cell::dim(r.date.as_deref().unwrap_or("")),
    }
}

fn row(spec: &BoardSpec, r: &LeaderboardRow, me: bool) -> Row {
    let row = Row::new(spec.columns.iter().map(|s| cell(*s, r, me)).collect());
    if me { row.accent() } else { row }
}

/// One board: its title (when side by side; tabs already name it), then
/// the table, or why there isn't one.
fn render_pane(
    frame: &mut Frame,
    pane: &BoardPane,
    spec: &BoardSpec,
    title: bool,
    focused: bool,
    area: Rect,
    p: &Palette,
) {
    let mut table_area = area;
    if title {
        let style = if focused { p.main_bold() } else { p.sub() };
        frame.render_widget(
            Paragraph::new(spec.label).style(style),
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
        .map(|r| row(spec, r, Some(r.result_id) == my_id))
        .collect();
    let columns: Vec<Column> = spec.columns.iter().map(|s| column(*s)).collect();
    let mut table = SelectTable::new(&columns, rows, &pane.selection).focused(focused);
    if let Some(me) = &pane.me {
        // Off-screen (or not fetched yet): pin at the bottom.
        let index = pane.my_index().unwrap_or(usize::MAX);
        table = table.pinned(index, row(spec, me, true));
    }
    table.render(frame, table_area, p);
}
