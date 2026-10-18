//! Bottom-line slider for numeric settings:
//! `words per line  ◂ ━━━━━●━━━━━━━━ ▸  13   ←/→ adjust · enter apply · esc cancel`

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{Palette, content_column};
use crate::app::App;

const TRACK: usize = 24;

/// Where the knob sits on the track; a value outside `min..=max` (from a
/// hand-edited config) pins it to an end instead of overflowing.
fn knob(value: u8, min: u8, max: u8) -> usize {
    let span = max.saturating_sub(min).max(1) as usize;
    let at = value.clamp(min, max.max(min)) - min;
    at as usize * (TRACK - 1) / span
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let Some(s) = app.slider else {
        return;
    };
    let col = content_column(area, app.screen_width());
    let bottom = Rect::new(col.x, area.bottom().saturating_sub(1), col.width, 1);

    let knob = knob(s.value, s.min(), s.max());
    let mut track = String::with_capacity(TRACK * 3);
    for i in 0..TRACK {
        track.push(if i == knob { '●' } else { '━' });
    }
    let (filled, rest) = track.split_at(
        track
            .char_indices()
            .nth(knob + 1)
            .map_or(track.len(), |(b, _)| b),
    );

    let line = Line::from(vec![
        Span::styled(format!("{}  ", s.target.label()), p.sub()),
        Span::styled("◂ ", p.sub()),
        Span::styled(filled.to_string(), p.main()),
        Span::styled(rest.to_string(), p.sub()),
        Span::styled(" ▸  ", p.sub()),
        Span::styled(format!("{:<3}", s.value), p.main_bold()),
        Span::styled("  ←/→ · enter · esc", p.sub()),
    ]);
    frame.render_widget(Paragraph::new(line), bottom);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_stays_on_the_track() {
        assert_eq!(knob(1, 1, 16), 0);
        assert_eq!(knob(16, 1, 16), TRACK - 1);
        // Out of range either way: pinned to an end, no overflow.
        assert_eq!(knob(0, 1, 16), 0);
        assert_eq!(knob(99, 1, 16), TRACK - 1);
    }
}
