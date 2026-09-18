//! Rasterizes an 8×8 bitmap font into block characters so the typing text
//! can be shown larger than the terminal font. Each glyph is drawn into a
//! fixed cell box given by `FontSize::cell_dims`; the style's foreground
//! paints "on" pixels and its background fills the box (which is how the
//! inverted caret works).

use font8x8::{BASIC_FONTS, LATIN_FONTS, UnicodeFonts};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::config::FontSize;

/// Quadrant block for the 4-bit pattern `tl<<3 | tr<<2 | bl<<1 | br`.
const QUADRANTS: [char; 16] = [
    ' ', '▗', '▖', '▄', '▝', '▐', '▞', '▟', '▘', '▚', '▌', '▙', '▀', '▜', '▛', '█',
];

fn bitmap(ch: char) -> [u8; 8] {
    BASIC_FONTS
        .get(ch)
        .or_else(|| LATIN_FONTS.get(ch))
        // Unknown glyph: a hollow box so it's obviously a placeholder.
        .unwrap_or([0x7e, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7e, 0x00])
}

fn pixel(bm: &[u8; 8], x: u16, y: u16) -> bool {
    if x >= 8 || y >= 8 {
        return false;
    }
    (bm[y as usize] >> x) & 1 == 1
}

/// Draw `ch` with its top-left at (`x`, `y`), clipped to `clip`.
pub fn draw_glyph(
    buf: &mut Buffer,
    clip: Rect,
    x: u16,
    y: u16,
    ch: char,
    style: Style,
    size: FontSize,
) {
    let (cols, rows) = size.cell_dims();
    let bm = if ch == ' ' { [0u8; 8] } else { bitmap(ch) };
    // Pixels per cell in each direction.
    let (px, py) = (8 / cols, 8 / rows);
    for cy in 0..rows {
        for cx in 0..cols {
            let (sx, sy) = (x + cx, y + cy);
            if sx < clip.x || sx >= clip.right() || sy < clip.y || sy >= clip.bottom() {
                continue;
            }
            let glyph = match (px, py) {
                (2, 2) => {
                    let tl = pixel(&bm, cx * 2, cy * 2) as usize;
                    let tr = pixel(&bm, cx * 2 + 1, cy * 2) as usize;
                    let bl = pixel(&bm, cx * 2, cy * 2 + 1) as usize;
                    let br = pixel(&bm, cx * 2 + 1, cy * 2 + 1) as usize;
                    QUADRANTS[tl << 3 | tr << 2 | bl << 1 | br]
                }
                (1, 2) => match (pixel(&bm, cx, cy * 2), pixel(&bm, cx, cy * 2 + 1)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                },
                _ => {
                    if pixel(&bm, cx, cy) {
                        '█'
                    } else {
                        ' '
                    }
                }
            };
            if let Some(cell) = buf.cell_mut((sx, sy)) {
                cell.set_char(glyph).set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_have_ink_and_spaces_do_not() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 4));
        let clip = buf.area;
        draw_glyph(&mut buf, clip, 0, 0, 'a', Style::default(), FontSize::Half);
        let ink = buf.content.iter().filter(|c| c.symbol() != " ").count();
        assert!(ink > 4, "expected a rendered glyph, got {ink} cells");

        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 4));
        draw_glyph(&mut buf, clip, 0, 0, ' ', Style::default(), FontSize::Half);
        assert!(buf.content.iter().all(|c| c.symbol() == " "));
    }

    #[test]
    fn clips_to_area() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 4));
        let clip = Rect::new(0, 0, 2, 2);
        draw_glyph(
            &mut buf,
            clip,
            0,
            0,
            'W',
            Style::default(),
            FontSize::Quadrant,
        );
        assert_eq!(buf[(3, 3)].symbol(), " ");
    }
}
