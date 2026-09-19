//! Bottom `:` line with a suggestion popup stacked above it.

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use crate::app::App;

/// First row the command line and its suggestions occupy.
pub fn popup_top(app: &App, area: Rect) -> u16 {
    let bottom = area.bottom().saturating_sub(1);
    bottom - app.cmdline.suggestions.len().min(bottom as usize) as u16
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.screen_width());
    let bottom = Rect::new(col.x, area.bottom().saturating_sub(1), col.width, 1);
    let cl = &app.cmdline;

    // Suggestions grow upward from the input line.
    let n = cl.suggestions.len().min(bottom.y as usize);
    let label_w = cl
        .suggestions
        .iter()
        .map(|s| s.label.chars().count())
        .max()
        .unwrap_or(0);
    for (i, s) in cl.suggestions.iter().take(n).enumerate() {
        let y = bottom.y - n as u16 + i as u16;
        let selected = i == cl.selected;
        let marker = if selected { "› " } else { "  " };
        let label_style = if selected { p.selected() } else { p.fg() };
        let line = Line::from(vec![
            Span::styled(marker, p.main()),
            Span::styled(format!("{:<label_w$}", s.label), label_style),
            Span::styled(format!("   {}", s.detail), p.sub()),
        ]);
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(bottom.x, y, bottom.width, 1),
        );
    }

    let line = Line::from(vec![
        Span::styled(":", p.main()),
        Span::styled(cl.input.clone(), p.fg()),
    ]);
    frame.render_widget(Paragraph::new(line), bottom);
    let cursor_x = bottom.x + 1 + cl.cursor as u16;
    frame.set_cursor_position(Position::new(
        cursor_x.min(bottom.right().saturating_sub(1)),
        bottom.y,
    ));
}
