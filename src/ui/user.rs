//! Profile screen: a player's streak, personal bests per daily mode and
//! latest runs, side by side when the terminal is wide enough, otherwise
//! stacked. The recent runs take the cursor; enter opens a run's graph.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ttyp_core::api::ProfileRun;

use super::style::{GUTTER, Palette, content_column};
use super::widgets::{Cell, Column, Panes, Row, SelectTable, Selection, Width, hints, panes};
use crate::app::App;

/// Narrowest a table may be before the two stack.
const MIN_PANE: u16 = 44;
const PANE_GAP: u16 = 4;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, area.width.saturating_sub(GUTTER * 2));
    let Some(view) = &app.user else {
        return;
    };
    let title_area = Rect::new(col.x, col.y + 2, col.width, 1);
    let body = Rect::new(col.x, col.y + 4, col.width, col.height.saturating_sub(7));

    let Some(profile) = &view.profile else {
        frame.render_widget(
            Paragraph::new(Span::styled(view.login.clone(), p.main_bold())),
            title_area,
        );
        let (msg, style) = match &view.error {
            Some(e) => (e.as_str(), p.error()),
            None => ("loading…", p.sub()),
        };
        frame.render_widget(Paragraph::new(msg).style(style), body);
        hints::render(frame, area, col, p, "esc back");
        return;
    };

    let visibility = if profile.public { "public" } else { "private" };
    let title = Line::from(vec![
        Span::styled(profile.login.clone(), p.main_bold()),
        Span::styled(
            format!("  ·  {visibility} · joined {}", profile.joined),
            p.sub(),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), title_area);
    let stats = format!(
        "streak {} · {} {}",
        profile.streak,
        profile.dailies,
        if profile.dailies == 1 {
            "daily"
        } else {
            "dailies"
        }
    );
    let sw = stats.chars().count() as u16;
    if col.width > sw + 40 {
        frame.render_widget(
            Paragraph::new(stats).style(p.fg()),
            Rect::new(col.right().saturating_sub(sw), title_area.y, sw, 1),
        );
    }

    let (bests_area, recent_area) = match panes::split(body, 2, MIN_PANE, PANE_GAP) {
        Panes::SideBySide(r) => (r[0], r[1]),
        Panes::Tabs(r) => {
            // Stacked: recent (the one you move through) on top.
            let half = r.height / 2;
            (
                Rect::new(
                    r.x,
                    r.y + half + 1,
                    r.width,
                    r.height.saturating_sub(half + 1),
                ),
                Rect::new(r.x, r.y, r.width, half),
            )
        }
    };

    let bests_sel = Selection::clamped(profile.bests.len());
    table(
        frame,
        "personal bests",
        &profile.bests,
        &bests_sel,
        false,
        false,
        bests_area,
        p,
    );
    table(
        frame,
        "recent",
        &profile.recent,
        &view.selection,
        true,
        true,
        recent_area,
        p,
    );
    hints::render(frame, area, col, p, "enter graph · ↑↓ move · esc back");
}

/// One titled table of runs; `recent` adds the attempt column and puts the
/// date first.
#[allow(clippy::too_many_arguments)]
fn table(
    frame: &mut Frame,
    title: &str,
    runs: &[ProfileRun],
    selection: &Selection,
    focused: bool,
    recent: bool,
    area: Rect,
    p: &Palette,
) {
    let style = if focused { p.main_bold() } else { p.sub() };
    frame.render_widget(
        Paragraph::new(title.to_string()).style(style),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if runs.is_empty() {
        frame.render_widget(
            Paragraph::new("  no dailies yet").style(p.sub()),
            Rect::new(body.x, body.y + 1, body.width, 1),
        );
        return;
    }
    let mut columns = vec![
        Column::new("language", Width::Min(8)).highlight(),
        Column::new("mode", Width::Fixed(8)),
        Column::new("wpm", Width::Fixed(4)).right(),
        Column::new("acc", Width::Fixed(4)).right(),
        Column::new("date", Width::Fixed(10)),
    ];
    if recent {
        columns.rotate_right(1);
        columns.push(Column::new("try", Width::Fixed(3)).right());
    }
    let rows = runs
        .iter()
        .map(|r| {
            let mut cells = vec![
                Cell::normal(&r.language),
                Cell::dim(r.mode.label()),
                Cell::normal(format!("{:.0}", r.wpm)),
                Cell::dim(format!("{:.0}%", r.acc)),
                Cell::dim(&r.date),
            ];
            if recent {
                cells.rotate_right(1);
                cells.push(Cell::dim(r.attempt.to_string()));
            }
            Row::new(cells)
        })
        .collect();
    SelectTable::new(&columns, rows, selection)
        .focused(focused)
        .render(frame, body, p);
}
