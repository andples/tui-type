//! Graph view of one leaderboard run: headline numbers and the per-second
//! chart, the same one the results screen draws.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::results::render_chart;
use super::style::{Palette, content_column, vcenter};
use super::widgets::hints;
use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let Some(d) = &app.graph else {
        return;
    };
    let col = content_column(area, app.config.content_width());
    // title (1) + gap + headline (2) + gap + detail (1) + gap + chart (10)
    let block = vcenter(col, 1 + 1 + 2 + 1 + 1 + 1 + 10);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(10),
    ])
    .split(block);

    let attempt = match d.attempt {
        1 => "first try".to_string(),
        n => format!("attempt {n}"),
    };
    let title = Line::from(vec![
        Span::styled(d.user.clone(), p.main_bold()),
        Span::styled(
            format!(
                "  ·  {} · {} · {}  ·  {attempt}",
                d.daily.language,
                d.daily.mode.label(),
                d.daily.date
            ),
            p.sub(),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), rows[0]);

    let label = Line::from(vec![
        Span::styled("wpm", p.sub()),
        Span::raw("        "),
        Span::styled("acc", p.sub()),
    ]);
    let value = Line::from(vec![
        Span::styled(format!("{:<8}", format!("{:.0}", d.wpm)), p.main_bold()),
        Span::raw("   "),
        Span::styled(format!("{:.0}%", d.acc), p.main_bold()),
    ]);
    frame.render_widget(Paragraph::new(vec![label, value]), rows[2]);

    let c = d.chars;
    let detail = Line::from(vec![
        Span::styled("raw ", p.sub()),
        Span::styled(format!("{:.0}", d.raw), p.fg()),
        Span::styled("  ·  con ", p.sub()),
        Span::styled(format!("{:.0}%", d.consistency), p.fg()),
        Span::styled("  ·  chars ", p.sub()),
        Span::styled(
            format!("{}/{}/{}/{}", c.correct, c.incorrect, c.extra, c.missed),
            p.fg(),
        ),
    ]);
    frame.render_widget(Paragraph::new(detail), rows[4]);

    render_chart(
        frame,
        &d.raw_per_second,
        &d.wpm_per_second,
        d.daily.mode,
        rows[6],
        p,
    );
    hints::render(frame, area, col, p, "esc back");
}
