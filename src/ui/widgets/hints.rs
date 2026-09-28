//! The key hint line at the bottom of a screen, just above the notice row.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::ui::style::Palette;

/// Draws `text` in the subdued colour on the second-to-last row of `area`,
/// inside the content column `col`.
pub fn render(frame: &mut Frame, area: Rect, col: Rect, p: &Palette, text: &str) {
    let row = Rect::new(col.x, area.bottom().saturating_sub(2), col.width, 1);
    frame.render_widget(Paragraph::new(text).style(p.sub()), row);
}
