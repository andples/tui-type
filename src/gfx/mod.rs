//! Real-font rendering for enlarged text via the kitty graphics protocol
//! (kitty, Ghostty, …). Every distinct glyph (character + colours) is
//! rasterized and uploaded once; each frame then only adds or removes
//! placements of those images, so a keystroke costs a few hundred bytes.
//! When images aren't available the UI falls back to block characters.

pub mod fonts;
pub mod kitty;
pub mod raster;

use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use ratatui::layout::Rect;

use crate::config::{Config, FontSize, Graphics};
use raster::Face;
pub use raster::{Glyph, LineGeom, Rgb};

/// Image ids are ours to pick; stay clear of small numbers other tools use.
const ID_BASE: u32 = 0x7474_7000;
/// Uploaded glyph images kept before starting afresh (theme previews can
/// otherwise pile up colour variants).
const MAX_IMAGES: usize = 1500;
/// Bytes of encoded glyph images kept for re-uploading.
const MAX_ENCODED_BYTES: usize = 8 << 20;

/// A request to draw one line of text over `area`.
#[derive(Debug, Clone)]
pub struct ImageLine {
    pub area: Rect,
    pub glyphs: Vec<Glyph>,
}

/// Pixel metrics the typing screen lays text out with.
#[derive(Debug, Clone, Copy)]
pub struct TextMetrics {
    pub geom: LineGeom,
    pub cell_w: u16,
    pub cell_h: u16,
}

/// Everything glyph images depend on besides the glyph itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RenderKey {
    px: u32,
    height: u32,
    cell: (u16, u16),
    generation: u64,
}

/// A glyph image ready to upload (`kitty::encode`).
struct Encoded {
    w: u32,
    h: u32,
    data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placed {
    image: u32,
    pid: u32,
}

/// The loaded font. At startup it loads on a thread (resolving a system
/// font runs `fc-match`, and a big font takes a while to parse), so the
/// first frame doesn't wait for it unless that frame draws with it.
#[derive(Default)]
struct FaceSlot {
    face: OnceCell<Option<Face>>,
    loading: RefCell<Option<JoinHandle<Result<Face, String>>>>,
    /// Why the background load failed, until `Gfx::take_load_error`.
    error: RefCell<Option<String>>,
}

impl FaceSlot {
    fn ready(face: Face) -> Self {
        Self {
            face: OnceCell::from(Some(face)),
            ..Default::default()
        }
    }

    fn background(spec: String, fonts_dir: PathBuf) -> Self {
        let handle = std::thread::spawn(move || load(&spec, &fonts_dir));
        Self {
            loading: RefCell::new(Some(handle)),
            ..Default::default()
        }
    }

    /// The face, waiting for a background load to finish if one is running.
    fn get(&self) -> Option<&Face> {
        self.face
            .get_or_init(|| {
                let handle = self.loading.borrow_mut().take()?;
                let result = handle
                    .join()
                    .unwrap_or_else(|_| Err("loading the font failed".into()));
                result.map_err(|e| *self.error.borrow_mut() = Some(e)).ok()
            })
            .as_ref()
    }

    fn get_mut(&mut self) -> Option<&mut Face> {
        self.get();
        self.face.get_mut()?.as_mut()
    }

    /// No face, and none on the way.
    fn is_none(&self) -> bool {
        self.face.get().is_some_and(Option::is_none)
            || (self.face.get().is_none() && self.loading.borrow().is_none())
    }

    /// A background load is still running.
    fn is_loading(&self) -> bool {
        self.loading
            .borrow()
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }
}

pub struct Gfx {
    supported: bool,
    face: FaceSlot,
    /// The `font` setting `face` was loaded from.
    font_spec: Option<String>,
    /// Bumped on every font load so old glyph images are dropped.
    generation: u64,
    /// Cell size in pixels.
    cell: Option<(u16, u16)>,
    /// What the uploaded images were rendered for.
    render_key: Option<RenderKey>,
    images: HashMap<Glyph, u32>,
    /// Every glyph rendered for `encoded_for` (px bits, height,
    /// generation), kept across purges: a resize clears the terminal's
    /// images, but they look the same and only need sending again.
    encoded: HashMap<Glyph, Encoded>,
    encoded_for: Option<(u32, u32, u64)>,
    encoded_bytes: usize,
    /// Live placements by absolute pixel position.
    placed: HashMap<(i64, u32), Placed>,
    /// Pixel width of each uploaded image, for cropping at the left edge.
    widths: HashMap<u32, u32>,
    next_image: u32,
    next_pid: u32,
}

impl Gfx {
    pub fn disabled() -> Self {
        Self {
            supported: false,
            face: FaceSlot::default(),
            font_spec: None,
            generation: 0,
            cell: None,
            render_key: None,
            images: HashMap::new(),
            encoded: HashMap::new(),
            encoded_for: None,
            encoded_bytes: 0,
            placed: HashMap::new(),
            widths: HashMap::new(),
            next_image: ID_BASE,
            next_pid: 1,
        }
    }

    /// Images on, with the first bundled font and `cell`-pixel cells.
    #[cfg(test)]
    pub(crate) fn for_tests(cell: (u16, u16)) -> Self {
        let mut g = Self::disabled();
        g.supported = true;
        g.cell = Some(cell);
        g.face = FaceSlot::ready(Face::from_files(fonts::BUNDLED[0].files).unwrap());
        g
    }

    /// Apply the `graphics` and `font` settings. On error images stay off
    /// and the caller should tell the user why.
    pub fn configure(&mut self, config: &Config, fonts_dir: &Path) -> Result<(), String> {
        self.configure_with(config, fonts_dir, false)
    }

    /// `configure` for startup: the font loads on a thread, and a failure
    /// is reported later by `take_load_error`.
    pub fn configure_in_background(
        &mut self,
        config: &Config,
        fonts_dir: &Path,
    ) -> Result<(), String> {
        self.configure_with(config, fonts_dir, true)
    }

    fn configure_with(
        &mut self,
        config: &Config,
        fonts_dir: &Path,
        background: bool,
    ) -> Result<(), String> {
        self.supported = detect(config.graphics);
        if !self.supported {
            return Ok(());
        }
        self.refresh_cell_size();
        if self.face.is_none() || self.font_spec.as_deref() != Some(config.font.as_str()) {
            self.face = FaceSlot::default();
            self.font_spec = None;
            self.face = if background {
                FaceSlot::background(config.font.clone(), fonts_dir.to_path_buf())
            } else {
                FaceSlot::ready(load(&config.font, fonts_dir)?)
            };
            self.font_spec = Some(config.font.clone());
            self.generation += 1;
        }
        if self.cell.is_none() {
            return Err("terminal did not report its size in pixels; using block glyphs".into());
        }
        Ok(())
    }

    /// Re-read the cell size (after a resize).
    pub fn refresh_cell_size(&mut self) {
        self.cell = crossterm::terminal::window_size().ok().and_then(|s| {
            let (cw, ch) = (
                s.width.checked_div(s.columns)?,
                s.height.checked_div(s.rows)?,
            );
            (cw > 0 && ch > 0).then_some((cw, ch))
        });
    }

    /// The startup font is still loading.
    pub fn loading(&self) -> bool {
        self.face.is_loading()
    }

    /// Why the startup font didn't load, once it has finished trying
    /// (waits for it when `wait`); reported once.
    pub fn take_load_error(&mut self, wait: bool) -> Option<String> {
        if !wait && self.face.is_loading() {
            return None;
        }
        self.face.get();
        let e = self.face.error.get_mut().take()?;
        self.font_spec = None;
        Some(e)
    }

    /// Characters drawn since the last call that the font has no glyph for
    /// (each reported once per font).
    pub fn take_missing(&mut self) -> Vec<char> {
        // Never waits for a background load: nothing was drawn before it.
        self.face
            .face
            .get_mut()
            .and_then(Option::as_mut)
            .map(Face::take_missing)
            .unwrap_or_default()
    }

    /// Whether the terminal speaks the protocol, even if no font loaded.
    pub fn supported(&self) -> bool {
        self.supported
    }

    /// Metrics for drawing `size` as images, or `None` to use cells.
    pub fn metrics(&self, size: FontSize) -> Option<TextMetrics> {
        if size.is_native() || !self.supported {
            return None;
        }
        let (face, (cell_w, cell_h)) = (self.face.get()?, self.cell?);
        let (_, rows) = size.cell_dims();
        Some(TextMetrics {
            geom: face.geom(rows as u32 * cell_h as u32),
            cell_w,
            cell_h,
        })
    }

    pub fn advance(&self, ch: char, px: f32) -> f32 {
        self.face.get().map_or(0.0, |f| f.advance(ch, px))
    }

    /// Measures many characters at `px` (`None` without a font).
    pub fn advancer(&self, px: f32) -> Option<raster::Advancer<'_>> {
        self.face.get().map(|f| f.advancer(px))
    }

    pub fn mean_advance(&self, px: f32) -> f32 {
        self.face.get().map_or(0.0, |f| f.mean_advance(px))
    }

    /// Make the screen show exactly `lines`, sending only the difference.
    pub fn present(&mut self, lines: &[ImageLine], out: &mut impl Write) -> io::Result<()> {
        let mut buf = Vec::new();
        let want = self.layout(lines, &mut buf);

        // Upload glyphs not seen before (with nothing to show, the font,
        // which may still be loading, isn't needed).
        if !want.is_empty()
            && let (Some(key), Some(face)) = (self.render_key, self.face.get_mut())
        {
            let geom = face.geom(key.height);
            let made_for = (key.px, key.height, key.generation);
            if self.encoded_for != Some(made_for) || self.encoded_bytes > MAX_ENCODED_BYTES {
                self.encoded.clear();
                self.encoded_bytes = 0;
                self.encoded_for = Some(made_for);
            }
            let fresh: HashSet<Glyph> = want
                .values()
                .filter(|g| !self.images.contains_key(g))
                .copied()
                .collect();
            for g in fresh {
                let bytes = &mut self.encoded_bytes;
                let img = self.encoded.entry(g).or_insert_with(|| {
                    let img = face.render_glyph(&g, geom);
                    let data = kitty::encode(&img.rgba);
                    *bytes += data.len();
                    Encoded {
                        w: img.w,
                        h: img.h,
                        data,
                    }
                });
                let id = self.next_image;
                self.next_image = self.next_image.wrapping_add(1).max(ID_BASE);
                kitty::transmit_encoded(&mut buf, id, img.w, img.h, &img.data);
                self.images.insert(g, id);
                self.widths.insert(id, img.w);
            }
        }

        // Drop placements that moved or changed, then add the new ones.
        let images = &self.images;
        self.placed.retain(|pos, p| {
            let keep = want.get(pos).and_then(|g| images.get(g)) == Some(&p.image);
            if !keep {
                kitty::delete_placement(&mut buf, p.image, p.pid);
            }
            keep
        });
        let (cw, ch) = self.cell.map_or((1, 1), |(w, h)| (w as u32, h as u32));
        let mut moved_cursor = false;
        for (pos, g) in &want {
            if self.placed.contains_key(pos) {
                continue;
            }
            let Some(&image) = self.images.get(g) else {
                continue;
            };
            if !moved_cursor {
                buf.extend_from_slice(kitty::SAVE_CURSOR);
                moved_cursor = true;
            }
            let pid = self.next_pid;
            self.next_pid = self.next_pid.wrapping_add(1).max(1);
            let (x, y) = *pos;
            // A glyph's left padding can reach past the screen edge; show
            // the part that's on screen rather than shifting it right.
            let (x, crop) = if x < 0 {
                let cut = x.unsigned_abs() as u32;
                let w = self.widths.get(&image).copied().unwrap_or(0);
                if cut >= w {
                    continue;
                }
                (0, Some((cut, w - cut)))
            } else {
                (x as u32, None)
            };
            kitty::place(&mut buf, image, pid, x / cw, y / ch, x % cw, y % ch, crop);
            self.placed.insert(*pos, Placed { image, pid });
        }
        if moved_cursor {
            buf.extend_from_slice(kitty::RESTORE_CURSOR);
        }

        if self.images.len() > MAX_IMAGES {
            self.render_key = None;
        }
        if !buf.is_empty() {
            out.write_all(&buf)?;
            out.flush()?;
        }
        Ok(())
    }

    /// Where every visible glyph goes, in absolute pixels. Purges uploaded
    /// images if they were rendered for a different size or font.
    fn layout(&mut self, lines: &[ImageLine], buf: &mut Vec<u8>) -> HashMap<(i64, u32), Glyph> {
        let mut want = HashMap::new();
        // No lines, no need for the font (which may still be loading).
        let Some(first) = lines.first() else {
            return want;
        };
        let (Some(face), Some((cw, ch))) = (self.face.get(), self.cell) else {
            return want;
        };
        let height = first.area.height as u32 * ch as u32;
        let geom = face.geom(height);
        let key = RenderKey {
            px: geom.px.to_bits(),
            height,
            cell: (cw, ch),
            generation: self.generation,
        };
        let pad = geom.pad();
        let advancer = face.advancer(geom.px);
        for line in lines {
            let (x0, y0) = (
                line.area.x as u32 * cw as u32,
                line.area.y as u32 * ch as u32,
            );
            let mut pen = 0.0f32;
            for g in &line.glyphs {
                let x = x0 as i64 + pen.round() as i64 - pad as i64;
                pen += advancer.advance(g.ch);
                if !g.is_blank() {
                    want.insert((x, y0), *g);
                }
            }
        }
        if self.render_key != Some(key) {
            self.purge(buf);
            self.render_key = Some(key);
        }
        want
    }

    /// Forget and delete every image (and so every placement).
    fn purge(&mut self, buf: &mut Vec<u8>) {
        for id in self.images.values() {
            kitty::delete(buf, *id);
        }
        self.images.clear();
        self.widths.clear();
        self.placed.clear();
    }

    /// Assume nothing on screen is current (e.g. the terminal was cleared).
    pub fn invalidate(&mut self) {
        self.render_key = None;
    }

    /// Remove every image we placed.
    pub fn clear(&mut self, out: &mut impl Write) -> io::Result<()> {
        let mut buf = Vec::new();
        self.purge(&mut buf);
        self.render_key = None;
        out.write_all(&buf)?;
        out.flush()
    }
}

fn load(spec: &str, fonts_dir: &Path) -> Result<Face, String> {
    let result = load_checked(spec, fonts_dir);
    if spec.trim().is_empty()
        && let Err(e) = &result
    {
        // No font was asked for, so any working one will do.
        let fallback = fonts::BUNDLED[0].name;
        crate::errlog::log(
            crate::errlog::Level::Warn,
            "font",
            format!("{e}; using bundled {fallback}"),
        );
        return load_checked(fallback, fonts_dir);
    }
    result
}

/// Load the font `spec` names and check it can draw text.
fn load_checked(spec: &str, fonts_dir: &Path) -> Result<Face, String> {
    let source = fonts::resolve(spec, fonts_dir)?;
    let files = source.load()?;
    let refs: Vec<&[u8]> = files.iter().map(Vec::as_slice).collect();
    let name = || match &source {
        fonts::Source::Bundled(name) => format!("font `{name}`"),
        fonts::Source::File(path) if spec.trim().is_empty() => {
            format!("system monospace font {}", path.display())
        }
        fonts::Source::File(path) if Path::new(spec.trim()) == path => {
            format!("font {}", path.display())
        }
        fonts::Source::File(path) => format!("font `{}` ({})", spec.trim(), path.display()),
    };
    let face = Face::from_files(&refs).map_err(|e| format!("{} can't be read: {e}", name()))?;
    // A font can parse and still have no letters (a symbols, icon or emoji
    // font, or the wrong face of a collection): every glyph would be a box.
    let missing: String = ('a'..='z')
        .chain('A'..='Z')
        .filter(|&c| !face.covers(c))
        .collect();
    match missing.len() {
        0 => Ok(face),
        26.. => Err(format!(
            "{} has no Latin letters (text would be boxes); using block glyphs, try :font",
            name()
        )),
        _ => Err(format!(
            "{} lacks the letters {missing}; using block glyphs, try :font",
            name()
        )),
    }
}

/// Terminals known to implement the kitty graphics protocol. Anything else,
/// including multiplexers (which swallow the escapes), falls back to block
/// glyphs.
fn detect(mode: Graphics) -> bool {
    match mode {
        Graphics::Off => false,
        Graphics::Kitty => {
            let var = |k: &str| std::env::var(k).unwrap_or_default();
            let term = var("TERM");
            if !var("TMUX").is_empty() || term.starts_with("screen") || term.starts_with("tmux") {
                return false;
            }
            term == "xterm-kitty"
                || term == "xterm-ghostty"
                || var("TERM_PROGRAM").eq_ignore_ascii_case("ghostty")
                || !var("KITTY_WINDOW_ID").is_empty()
                || !var("GHOSTTY_RESOURCES_DIR").is_empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gfx() -> Gfx {
        Gfx::for_tests((10, 20))
    }

    fn line(text: &str, caret: usize) -> ImageLine {
        ImageLine {
            area: Rect::new(4, 2, 60, 2),
            glyphs: text
                .chars()
                .enumerate()
                .map(|(i, ch)| Glyph {
                    ch,
                    fg: [200, 200, 200],
                    bg: (i == caret).then_some([255, 200, 0]),
                    underline: false,
                })
                .collect(),
        }
    }

    fn count(out: &[u8], pat: &str) -> usize {
        String::from_utf8_lossy(out).matches(pat).count()
    }

    #[test]
    fn glyphs_upload_once_and_frames_send_only_changes() {
        let mut g = gfx();
        let mut out = Vec::new();
        g.present(&[line("aa bb", 0)], &mut out).unwrap();
        // Caret 'a', plain 'a', 'b': three images; four placements.
        assert_eq!(count(&out, "a=t,"), 3);
        assert_eq!(count(&out, "a=p,"), 4);

        out.clear();
        g.present(&[line("aa bb", 0)], &mut out).unwrap();
        assert!(out.is_empty(), "unchanged frame sends nothing");

        // Caret moves right: both variants already exist, so two
        // placements are swapped and nothing is uploaded.
        out.clear();
        g.present(&[line("aa bb", 1)], &mut out).unwrap();
        assert_eq!(count(&out, "a=t,"), 0);
        assert_eq!(count(&out, "d=i,"), 2);
        assert_eq!(count(&out, "a=p,"), 2);
    }

    #[test]
    fn clearing_and_size_changes_drop_images() {
        let mut g = gfx();
        let mut out = Vec::new();
        g.present(&[line("ab", 0)], &mut out).unwrap();
        out.clear();
        g.present(&[], &mut out).unwrap();
        assert_eq!(count(&out, "d=i,"), 2);

        out.clear();
        g.present(&[line("ab", 0)], &mut out).unwrap();
        let mut taller = line("ab", 0);
        taller.area.height = 3;
        out.clear();
        g.present(&[taller], &mut out).unwrap();
        assert_eq!(count(&out, "d=I,"), 2);
        assert_eq!(count(&out, "a=t,"), 2);

        out.clear();
        g.clear(&mut out).unwrap();
        assert_eq!(count(&out, "d=I,"), 2);
        assert!(g.placed.is_empty());
    }

    #[test]
    fn a_font_without_letters_is_an_error_not_boxes() {
        let dir = Path::new("/nonexistent");
        // Fontsource's Latin Extended subset has no plain a-z.
        let ext = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/jetbrains-mono-latin-ext.ttf"
        );
        let e = load(ext, dir).err().expect("an error");
        assert!(e.contains("no Latin letters"), "{e}");
        assert!(e.contains("jetbrains-mono-latin-ext.ttf"), "{e}");

        let junk = std::env::temp_dir().join(format!("ttyp-junk-{}.ttf", std::process::id()));
        std::fs::write(&junk, b"not a font").unwrap();
        let e = load(junk.to_str().unwrap(), dir).err().expect("an error");
        assert!(e.contains("can't be read"), "{e}");
        std::fs::remove_file(&junk).unwrap();

        assert!(load("JetBrains Mono", dir).is_ok());
    }

    #[test]
    fn background_font_loads_on_first_use_and_reports_failure() {
        let dir = Path::new("/nonexistent");
        let mut g = gfx();
        g.face = FaceSlot::background("Fira Code".into(), dir.into());
        assert!(g.metrics(FontSize::from_level(5)).is_some());
        assert_eq!(g.take_load_error(true), None);

        g.face = FaceSlot::background("no such font, surely".into(), dir.into());
        g.font_spec = Some("no such font, surely".into());
        // Nothing to draw: the font isn't waited for.
        let mut out = Vec::new();
        g.present(&[], &mut out).unwrap();
        let e = g.take_load_error(true).expect("an error");
        assert!(e.contains("not found"), "{e}");
        assert!(g.metrics(FontSize::from_level(5)).is_none());
        assert_eq!(g.font_spec, None, "the same font is tried again");
        assert_eq!(g.take_load_error(true), None, "reported once");
    }

    #[test]
    fn a_resize_sends_the_same_images_again_without_redrawing_them() {
        let mut g = gfx();
        let mut first = Vec::new();
        g.present(&[line("ab", 0)], &mut first).unwrap();
        assert_eq!(g.encoded.len(), 2);
        // The terminal cleared (a resize): everything goes again.
        g.invalidate();
        let mut again = Vec::new();
        g.present(&[line("ab", 0)], &mut again).unwrap();
        assert_eq!(count(&again, "a=t,"), 2);
        let payloads = |out: &[u8]| {
            let mut p: Vec<String> = String::from_utf8_lossy(out)
                .split("\x1b_G")
                .filter(|seg| seg.starts_with("a=t,"))
                .map(|seg| {
                    let data = seg.split_once(';').unwrap().1;
                    data.split_once("\x1b\\").unwrap().0.to_string()
                })
                .collect();
            p.sort();
            p
        };
        assert_eq!(payloads(&first), payloads(&again));
        // A different size is drawn afresh.
        let mut taller = line("ab", 0);
        taller.area.height = 3;
        g.present(&[taller], &mut Vec::new()).unwrap();
        assert_eq!(g.encoded_for.map(|k| k.1), Some(60));
        assert_eq!(g.encoded.len(), 2);
    }

    #[test]
    fn placements_use_cell_plus_pixel_offset() {
        let mut g = gfx();
        let mut out = Vec::new();
        g.present(&[line("x", usize::MAX)], &mut out).unwrap();
        let (&(x, y), _) = g.placed.iter().next().unwrap();
        let pad = g.face.get().unwrap().geom(40).pad();
        assert_eq!((x, y), (40 - pad as i64, 40));
        let s = String::from_utf8_lossy(&out);
        assert!(
            s.contains(&format!("\x1b[3;{}H", (40 - pad) / 10 + 1)),
            "{s:?}"
        );
        assert!(s.contains(&format!("X={},Y=0", (40 - pad) % 10)));
    }

    #[test]
    fn glyphs_at_the_left_edge_are_cropped_not_shifted() {
        let mut g = gfx();
        let mut out = Vec::new();
        let mut l = line("x", usize::MAX);
        l.area.x = 0;
        g.present(&[l], &mut out).unwrap();
        let pad = g.face.get().unwrap().geom(40).pad();
        assert_eq!(g.placed.keys().next().unwrap().0, -(pad as i64));
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains(&format!("x={pad},w=")), "{s:?}");
        assert!(s.contains("\x1b[3;1H"), "{s:?}");
    }
}
