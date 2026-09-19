//! The test screen: timer/counter, a three-line scrolling word box with a
//! block caret, and a dim mode line.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::bigtext;
use super::style::{Palette, content_column, vcenter};
use crate::app::App;
use crate::gfx::{Glyph, ImageLine, Rgb};
use crate::test::{Mode, Status, Word};

const VISIBLE_LINES: usize = 3;

/// Which words go on which line when a line holds `max` units, a space is
/// `space` units and `width_of` measures a word. Words never wrap mid-word.
fn layout_lines(
    words: &[Word],
    max: f32,
    space: f32,
    width_of: impl Fn(&Word) -> f32,
) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut used = 0.0;
    for (i, w) in words.iter().enumerate() {
        let len = width_of(w);
        let needed = if i == start { len } else { used + space + len };
        if needed > max && i > start {
            lines.push((start, i));
            start = i;
            used = len;
        } else {
            used = needed;
        }
    }
    lines.push((start, words.len()));
    lines
}

/// One character cell of the word box: what to draw and how.
type Cell = (char, Style);

/// The characters a word shows: its target, then any extra typed ones.
fn shown_chars(w: &Word) -> impl Iterator<Item = char> + '_ {
    let n = w.target.len().max(w.typed.len());
    (0..n).filter_map(|j| w.target.get(j).or(w.typed.get(j)).copied())
}

fn rgb(c: Color, fallback: Rgb) -> Rgb {
    match c {
        Color::Rgb(r, g, b) => [r, g, b],
        _ => fallback,
    }
}

fn to_glyph((ch, st): Cell, p: &Palette) -> Glyph {
    let fg = rgb(p.fg, [255; 3]);
    Glyph {
        ch,
        fg: rgb(st.fg.unwrap_or(p.fg), fg),
        bg: st.bg.map(|c| rgb(c, fg)),
        underline: st.add_modifier.contains(Modifier::UNDERLINED),
    }
}

fn word_cells(w: &Word, is_current: bool, p: &Palette) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(w.target.len() + 2);
    let caret_at = if is_current {
        Some(w.typed.len())
    } else {
        None
    };
    let n = w.target.len().max(w.typed.len());
    for j in 0..n {
        let (ch, style) = match (w.target.get(j), w.typed.get(j)) {
            (Some(t), Some(ty)) if t == ty => (*t, p.correct()),
            (Some(t), Some(_)) => (*t, p.error()),
            (None, Some(ty)) => (*ty, p.error_extra()),
            (Some(t), None) => (*t, p.sub()),
            (None, None) => unreachable!(),
        };
        let style = if caret_at == Some(j) {
            p.caret()
        } else {
            style
        };
        cells.push((ch, style));
    }
    // Caret sits on the trailing space when the word is fully typed.
    let space_style = if caret_at == Some(n) {
        p.caret()
    } else {
        p.sub()
    };
    cells.push((' ', space_style));
    cells
}

/// Draws the typing screen. Returns the lines to show as real-font images
/// (empty unless the terminal supports them and the font size is > 1).
pub fn render(frame: &mut Frame, app: &App, area: Rect, p: &Palette) -> Vec<ImageLine> {
    let font = app.config.font_size();
    let metrics = app.gfx.metrics(font);
    let (gw, gh) = font.cell_dims();
    let col = content_column(area, app.typing_width());
    // Row stride per line: a gap row for big fonts.
    let stride = if gh > 1 { gh + 1 } else { 1 };
    let words = app.engine.words();
    let current = app.engine.current_index();

    let lines = match metrics {
        Some(m) => {
            let px = m.geom.px;
            layout_lines(
                words,
                col.width as f32 * m.cell_w as f32,
                app.gfx.advance(' ', px),
                |w| shown_chars(w).map(|c| app.gfx.advance(c, px)).sum(),
            )
        }
        None => layout_lines(words, (col.width / gw).max(1) as f32, 1.0, |w| {
            w.target.len().max(w.typed.len()) as f32
        }),
    };
    let current_line = lines
        .iter()
        .position(|(s, e)| current >= *s && current < *e)
        .unwrap_or(0);
    let first = current_line.saturating_sub(1);
    let visible: Vec<Vec<Cell>> = lines
        .iter()
        .skip(first)
        .take(VISIBLE_LINES)
        .map(|(s, e)| {
            words[*s..*e]
                .iter()
                .enumerate()
                .flat_map(|(k, w)| word_cells(w, s + k == current, p))
                .collect()
        })
        .collect();

    // header (1) + gap (1) + words + gap (1) + mode line (1)
    let words_h = VISIBLE_LINES as u16 * stride - (stride - gh);
    let block = vcenter(col, words_h + 4);
    let header = Rect::new(block.x, block.y, block.width, 1);
    let words_area = Rect::new(block.x, block.y + 2, block.width, words_h);
    let footer = Rect::new(block.x, block.y + 3 + words_h, block.width, 1);
    let zen = app.config.zen;

    let status = app.engine.status();
    let counter = match app.engine.mode() {
        Mode::Time(secs) => {
            let remaining = app
                .engine
                .remaining_at(Instant::now())
                .map(|d| d.as_secs_f64().ceil() as u64)
                .unwrap_or(secs as u64);
            if status == Status::Idle {
                secs.to_string()
            } else {
                remaining.to_string()
            }
        }
        Mode::Words(n) => format!("{}/{n}", app.engine.words_completed()),
    };
    let header_style = if status == Status::Running {
        p.main_bold()
    } else {
        p.sub()
    };
    if !zen {
        frame.render_widget(Paragraph::new(counter).style(header_style), header);
    }

    let mut images = Vec::new();
    if metrics.is_some() {
        for (row, cells) in visible.into_iter().enumerate() {
            let y = words_area.y + row as u16 * stride;
            if y + gh > words_area.bottom() {
                break;
            }
            images.push(ImageLine {
                area: Rect::new(words_area.x, y, words_area.width, gh),
                glyphs: cells.into_iter().map(|c| to_glyph(c, p)).collect(),
            });
        }
    } else if font.is_native() {
        let text: Vec<Line> = visible
            .into_iter()
            .map(|cells| {
                Line::from(
                    cells
                        .into_iter()
                        .map(|(c, st)| Span::styled(c.to_string(), st))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(text), words_area);
    } else {
        let buf = frame.buffer_mut();
        for (row, cells) in visible.iter().enumerate() {
            let y = words_area.y + row as u16 * stride;
            for (i, (c, st)) in cells.iter().enumerate() {
                let x = words_area.x + i as u16 * gw;
                bigtext::draw_glyph(buf, words_area, x, y, *c, *st, font);
            }
        }
    }

    // The mode line fades out while typing so nothing distracts from the words.
    if status != Status::Running && !zen {
        frame.render_widget(Paragraph::new(mode_line(app)).style(p.sub()), footer);
    }
    images
}

pub fn mode_line(app: &App) -> String {
    let c = &app.config;
    let lang = app.languages.get_or_default(&c.language);
    let mut parts = vec![c.mode.label(), lang.name.clone()];
    if c.punctuation {
        parts.push("punctuation".into());
    }
    if c.numbers {
        parts.push("numbers".into());
    }
    parts.join("  ·  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Word {
        Word {
            target: s.chars().collect(),
            typed: vec![],
        }
    }

    fn chars(w: &Word) -> f32 {
        w.target.len() as f32
    }

    #[test]
    fn wraps_on_word_boundaries() {
        let words = vec![w("aaaa"), w("bbbb"), w("cccc"), w("dd")];
        // "aaaa bbbb" = 9 fits in 10; "cccc" would make 14 → new line
        assert_eq!(layout_lines(&words, 10.0, 1.0, chars), vec![(0, 2), (2, 4)]);
    }

    #[test]
    fn long_word_gets_own_line() {
        let words = vec![w("a"), w("bbbbbbbbbbbbbbbb"), w("c")];
        assert_eq!(
            layout_lines(&words, 8.0, 1.0, chars),
            vec![(0, 1), (1, 2), (2, 3)]
        );
    }

    #[test]
    fn wraps_by_measured_width() {
        // Pixel layout: 10px per char, 4px spaces, 100px lines.
        let words = vec![w("aaaa"), w("bbbb"), w("cc")];
        let px = |w: &Word| w.target.len() as f32 * 10.0;
        assert_eq!(layout_lines(&words, 100.0, 4.0, px), vec![(0, 2), (2, 3)]);
        assert_eq!(layout_lines(&words, 84.0, 4.0, px), vec![(0, 2), (2, 3)]);
        assert_eq!(layout_lines(&words, 83.0, 4.0, px), vec![(0, 1), (1, 3)]);
    }
}
