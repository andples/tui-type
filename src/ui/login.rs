//! Login screen: the GitHub device code to enter, while the flow polls.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use super::widgets::hints;
use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let col = content_column(area, app.config.content_width());
    let body = Rect::new(col.x, col.y + 2, col.width, col.height.saturating_sub(5));
    let mut lines = vec![
        Line::from(Span::styled("login with github", p.main_bold())),
        Line::default(),
    ];
    match &app.login_prompt {
        Some((code, uri)) => {
            lines.push(Line::from(vec![
                Span::styled("  open  ", p.sub()),
                Span::styled(uri.clone(), p.fg()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  enter ", p.sub()),
                Span::styled(code.clone(), p.selected()),
            ]));
            lines.push(Line::default());
            lines.push(Line::from(Span::styled("  waiting for github…", p.sub())));
        }
        None => lines.push(Line::from(Span::styled("  requesting a code…", p.sub()))),
    }
    frame.render_widget(Paragraph::new(lines), body);
    hints::render(frame, area, col, p, "esc cancel");
}
