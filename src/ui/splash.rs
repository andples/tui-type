//! Landing screen: the logo in the big pixel font, typed out with a caret,
//! then a tagline and the keys that fit the online state. Falls back to a
//! one-row text logo when no block size fits.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::bigtext;
use super::style::{Palette, content_column, vcenter};
use crate::app::App;
use crate::app::splash::LOGO;
use crate::config::{BlockSet, FontSize};

const TAGLINE: &str = "a minimal typing test for the terminal";
/// Tallest logo outside fullscreen, in rows: calm, not a billboard.
const LOGO_ROWS: u16 = 8;

/// The largest block size at which the logo (plus a caret slot) fits in
/// `width` × `rows`; `None` when only the terminal font does. Half blocks
/// only: they draw crisply in every terminal font, unlike sextants.
fn logo_size(width: u16, rows: u16) -> Option<FontSize> {
    let slots = LOGO.len() as u16 + 1;
    (1..=9)
        .rev()
        .map(|s| FontSize::blocks(BlockSet::Half, s))
        .find(|f| {
            let (w, h) = f.cell_dims();
            h <= rows && w * slots <= width
        })
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) {
    let state = app.splash_frame();
    let items = app.splash_menu().items();
    let col = if app.config.fullscreen {
        Rect::new(
            area.x + 1,
            area.y,
            area.width.saturating_sub(2),
            area.height,
        )
    } else {
        content_column(area, app.config.content_width())
    };

    // Leave the brand row at the top and the hint line at the bottom.
    let usable = Rect::new(col.x, area.y + 1, col.width, area.height.saturating_sub(3));
    // Rows under the logo: a gap and the tagline, a gap and the keys. They
    // keep their room during the intro so the logo doesn't jump when they
    // appear. In a short terminal the tagline goes first, then the keys
    // (they still work).
    let menu_rows = 1 + items.len() as u16;
    let mut show_menu = !app.config.zen;
    let mut show_tagline = true;
    let below = |tagline: bool, menu: bool| 2 * tagline as u16 + menu_rows * menu as u16;
    if 1 + below(show_tagline, show_menu) > usable.height {
        show_tagline = false;
    }
    if 1 + below(show_tagline, show_menu) > usable.height {
        show_menu = false;
    }
    let below = below(show_tagline, show_menu);
    let max_rows = if app.config.fullscreen {
        usable.height / 2
    } else {
        LOGO_ROWS
    };
    let size = logo_size(col.width, usable.height.saturating_sub(below).min(max_rows));
    let logo_h = size.map_or(1, |f| f.cell_dims().1);
    let block = vcenter(usable, logo_h + below);

    let logo_area = Rect::new(block.x, block.y, block.width, logo_h);
    match size {
        Some(f) => draw_big_logo(frame, logo_area, f, state.letters, p),
        None => draw_text_logo(frame, logo_area, state.letters, p),
    }
    if !state.done {
        return;
    }

    let centred = |text: &'static str, y: u16| {
        let row = Rect::new(col.x, y, col.width, 1).intersection(area);
        (
            Paragraph::new(text)
                .style(p.sub())
                .alignment(Alignment::Center),
            row,
        )
    };
    let mut y = block.y + logo_h;
    if show_tagline {
        let (w, row) = centred(fitting(&[TAGLINE, "a typing test"], col.width), y + 1);
        frame.render_widget(w, row);
        y += 2;
    }
    if !show_menu {
        return;
    }
    y += 1;
    let key_w = items.iter().map(|(k, _)| k.len()).max().unwrap_or(0) + 3;
    let menu_w = items
        .iter()
        .map(|(_, l)| key_w + l.len())
        .max()
        .unwrap_or(0) as u16;
    let x = col.x + col.width.saturating_sub(menu_w) / 2;
    for (key, label) in items {
        let row = Rect::new(x, y, menu_w.min(col.width), 1).intersection(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{key:<key_w$}"), p.main()),
                Span::styled(*label, p.fg()),
            ])),
            row,
        );
        y += 1;
    }
    // The key line, centred like the rest of the screen.
    let hint = fitting(
        &["any key to start · : commands · ? help", "any key to start"],
        col.width,
    );
    let (w, row) = centred(hint, area.bottom().saturating_sub(2));
    frame.render_widget(w, row);
}

/// The first of `options` that fits in `width` columns, else nothing.
fn fitting(options: &[&'static str], width: u16) -> &'static str {
    options
        .iter()
        .find(|t| t.chars().count() <= width as usize)
        .copied()
        .unwrap_or("")
}

/// The logo in the pixel font, centred as four letters; the caret sits in
/// the slot after the last letter shown.
fn draw_big_logo(frame: &mut Frame, area: Rect, size: FontSize, letters: usize, p: &Palette) {
    let (gw, _) = size.cell_dims();
    let x0 = area.x + area.width.saturating_sub(gw * LOGO.len() as u16) / 2;
    let clip = area.intersection(frame.area());
    let buf = frame.buffer_mut();
    for (i, ch) in LOGO.chars().take(letters).enumerate() {
        let style = if i == 0 { p.main_bold() } else { p.sub() };
        bigtext::draw_glyph(buf, clip, x0 + i as u16 * gw, area.y, ch, style, size);
    }
    let caret_x = x0 + letters as u16 * gw;
    bigtext::draw_glyph(buf, clip, caret_x, area.y, '|', p.main(), size);
}

/// One row: the brand mark as text with a block caret.
fn draw_text_logo(frame: &mut Frame, area: Rect, letters: usize, p: &Palette) {
    let mut spans: Vec<Span> = LOGO
        .chars()
        .take(letters)
        .enumerate()
        .map(|(i, c)| {
            let style = if i == 0 { p.main_bold() } else { p.sub() };
            Span::styled(c.to_string(), style)
        })
        .collect();
    // Pad to the full word so the text doesn't shift as it types.
    spans.push(Span::styled(" ", p.caret()));
    spans.push(Span::raw(" ".repeat(LOGO.len() - letters)));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_shrinks_to_fit_then_gives_up() {
        let (w, h) = logo_size(80, 8).unwrap().cell_dims();
        assert!(h <= 8 && w * 5 <= 80);
        assert_eq!(logo_size(80, 8), Some(FontSize::from_level(5)));
        let small = logo_size(52, 6).unwrap().cell_dims();
        assert!(small.1 <= 6 && small.0 * 5 <= 52);
        assert_eq!(logo_size(80, 3), Some(FontSize::from_level(3)));
        assert_eq!(logo_size(19, 8), None, "too narrow for any block size");
        assert_eq!(logo_size(80, 2), None, "too short for any block size");
    }

    #[test]
    fn text_falls_back_to_what_fits() {
        assert_eq!(fitting(&["long one", "short"], 8), "long one");
        assert_eq!(fitting(&["long one", "short"], 7), "short");
        assert_eq!(fitting(&["long one", "short"], 4), "");
    }
}
