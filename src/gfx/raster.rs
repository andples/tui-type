//! Anti-aliased glyph rasterization from real font files.

use std::collections::HashMap;

use fontdue::{Font, FontSettings, Metrics};

pub type Rgb = [u8; 3];

/// Share of the line height the font's ascent+descent box fills.
const TEXT_FILL: f32 = 0.92;
/// Transparent margin each side of a glyph image, in ems, so overhanging
/// ink (italics, wide glyphs) isn't clipped.
const PAD_EM: f32 = 0.3;

/// One character on screen, with everything that affects how it looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Glyph {
    pub ch: char,
    pub fg: Rgb,
    /// Box behind the glyph (the caret).
    pub bg: Option<Rgb>,
    pub underline: bool,
}

impl Glyph {
    /// Whether drawing it would leave no mark.
    pub fn is_blank(&self) -> bool {
        self.ch.is_whitespace() && self.bg.is_none() && !self.underline
    }
}

/// Vertical geometry of a line of text `height` pixels tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineGeom {
    /// Font size in pixels per em.
    pub px: f32,
    pub height: u32,
    pub baseline: f32,
    /// Top and bottom of the ascent/descent box, for the caret.
    pub text_top: f32,
    pub text_bottom: f32,
}

impl LineGeom {
    pub fn pad(&self) -> u32 {
        (self.px * PAD_EM).ceil() as u32
    }
}

/// A rasterized glyph: RGBA, with `LineGeom::pad` transparent pixels left
/// of the pen position.
pub struct GlyphImage {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// A font plus fallbacks for characters it lacks (bundled fonts ship as
/// separate script subsets).
pub struct Face {
    fonts: Vec<Font>,
    bitmaps: HashMap<(char, u32), (Metrics, Vec<u8>)>,
}

impl Face {
    pub fn from_files(files: &[&[u8]]) -> Result<Self, String> {
        let fonts = files
            .iter()
            .map(|d| Font::from_bytes(*d, FontSettings::default()).map_err(str::to_string))
            .collect::<Result<Vec<_>, _>>()?;
        if fonts.is_empty() {
            return Err("no font data".into());
        }
        Ok(Self {
            fonts,
            bitmaps: HashMap::new(),
        })
    }

    fn font_for(&self, ch: char) -> &Font {
        self.fonts
            .iter()
            .find(|f| f.lookup_glyph_index(ch) != 0)
            .unwrap_or(&self.fonts[0])
    }

    pub fn geom(&self, height: u32) -> LineGeom {
        let (ascent, descent) = self.fonts[0]
            .horizontal_line_metrics(1.0)
            .map(|m| (m.ascent, m.descent))
            .filter(|(a, d)| a - d > 0.0)
            .unwrap_or((0.8, -0.2));
        let box_per_px = ascent - descent;
        let px = height as f32 * TEXT_FILL / box_per_px;
        let text_h = box_per_px * px;
        let text_top = (height as f32 - text_h) / 2.0;
        LineGeom {
            px,
            height,
            baseline: text_top + ascent * px,
            text_top,
            text_bottom: text_top + text_h,
        }
    }

    pub fn advance(&self, ch: char, px: f32) -> f32 {
        self.font_for(ch).metrics(ch, px).advance_width
    }

    /// Average lowercase advance: how wide a "typical" character is.
    pub fn mean_advance(&self, px: f32) -> f32 {
        ('a'..='z').map(|c| self.advance(c, px)).sum::<f32>() / 26.0
    }

    /// Draw one glyph with its caret box and underline, on transparency.
    pub fn render_glyph(&mut self, g: &Glyph, geom: LineGeom) -> GlyphImage {
        let adv = self.advance(g.ch, geom.px);
        let pad = geom.pad() as i32;
        let w = (2 * pad + adv.ceil() as i32).max(1) as usize;
        let h = geom.height as usize;
        let mut img = Canvas {
            w,
            h,
            px: vec![0; w * h * 4],
        };
        let x1 = pad + adv.round() as i32;
        if let Some(bg) = g.bg {
            let (top, bottom) = (
                geom.text_top.round() as i32,
                geom.text_bottom.round() as i32,
            );
            img.fill(pad, top, x1, bottom, bg);
        }
        if !g.ch.is_whitespace() {
            let (m, cov) = self.bitmap(g.ch, geom.px);
            let gx = pad + m.xmin;
            let gy = (geom.baseline - m.ymin as f32 - m.height as f32).round() as i32;
            img.over(gx, gy, m.width, m.height, cov, g.fg);
        }
        if g.underline {
            let t = (geom.px * 0.06).round().max(1.0) as i32;
            let y = (geom.baseline + geom.px * 0.12).round() as i32;
            img.fill(pad, y, x1, y + t, g.fg);
        }
        GlyphImage {
            w: w as u32,
            h: h as u32,
            rgba: img.px,
        }
    }

    fn bitmap(&mut self, ch: char, px: f32) -> &(Metrics, Vec<u8>) {
        let font = self
            .fonts
            .iter()
            .find(|f| f.lookup_glyph_index(ch) != 0)
            .unwrap_or(&self.fonts[0]);
        self.bitmaps
            .entry((ch, px.to_bits()))
            .or_insert_with(|| font.rasterize(ch, px))
    }
}

/// Straight-alpha RGBA pixels.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let clamp = |v: i32, max: usize| v.clamp(0, max as i32) as usize;
        let (x0, x1) = (clamp(x0, self.w), clamp(x1, self.w));
        let (y0, y1) = (clamp(y0, self.h), clamp(y1, self.h));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * self.w + x) * 4;
                self.px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }

    /// Composite a coverage mask in `fg` over what is there ("over" operator).
    fn over(&mut self, gx: i32, gy: i32, gw: usize, gh: usize, cov: &[u8], fg: Rgb) {
        for row in 0..gh {
            let y = gy + row as i32;
            if y < 0 || y >= self.h as i32 {
                continue;
            }
            for col in 0..gw {
                let x = gx + col as i32;
                if x < 0 || x >= self.w as i32 {
                    continue;
                }
                let a = cov[row * gw + col] as u32;
                if a == 0 {
                    continue;
                }
                let i = (y as usize * self.w + x as usize) * 4;
                let da = self.px[i + 3] as u32;
                // Output alpha and colour, both scaled by 255.
                let oa = a * 255 + da * (255 - a);
                for (k, &f) in fg.iter().enumerate() {
                    let d = self.px[i + k] as u32;
                    let v = (f as u32 * a * 255 + d * da * (255 - a) + oa / 2) / oa;
                    self.px[i + k] = v as u8;
                }
                self.px[i + 3] = ((oa + 127) / 255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            px: vec![0; w * h * 4],
        }
    }

    #[test]
    fn fill_is_opaque_and_clipped() {
        let mut c = canvas(4, 2);
        c.fill(-5, -5, 2, 1, [9, 9, 9]);
        assert_eq!(&c.px[0..8], &[9, 9, 9, 255, 9, 9, 9, 255]);
        assert_eq!(&c.px[8..], &[0; 24]);
    }

    #[test]
    fn over_transparent_keeps_colour_and_coverage_as_alpha() {
        let mut c = canvas(2, 1);
        c.over(1, 0, 2, 1, &[128, 255], [200, 100, 0]);
        assert_eq!(c.px, vec![0, 0, 0, 0, 200, 100, 0, 128]);
    }

    #[test]
    fn over_opaque_blends() {
        let mut c = canvas(1, 1);
        c.fill(0, 0, 1, 1, [0, 0, 0]);
        c.over(0, 0, 1, 1, &[128], [255, 255, 255]);
        assert_eq!(c.px, vec![128, 128, 128, 255]);
    }
}
