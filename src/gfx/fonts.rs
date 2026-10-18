//! Turning the `font` setting into font data. Lookup order: a file path,
//! a file in the config `fonts/` dir, a bundled font, then an installed
//! family via fontconfig (`fc-match`, Linux/BSD).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const EXTENSIONS: [&str; 3] = ["ttf", "otf", "ttc"];

/// A font compiled into the binary: Regular weight, as Latin and
/// Latin Extended subsets (from Fontsource; licenses in `assets/fonts/`).
pub struct Bundled {
    pub name: &'static str,
    pub files: &'static [&'static [u8]],
}

macro_rules! bundled {
    ($name:literal, $slug:literal) => {
        Bundled {
            name: $name,
            files: &[
                include_bytes!(concat!("../../assets/fonts/", $slug, "-latin.ttf")),
                include_bytes!(concat!("../../assets/fonts/", $slug, "-latin-ext.ttf")),
            ],
        }
    };
}

pub const BUNDLED: &[Bundled] = &[
    bundled!("JetBrains Mono", "jetbrains-mono"),
    bundled!("Fira Code", "fira-code"),
    bundled!("Source Code Pro", "source-code-pro"),
    bundled!("IBM Plex Mono", "ibm-plex-mono"),
    bundled!("Cascadia Code", "cascadia-code"),
    bundled!("Roboto Mono", "roboto-mono"),
    bundled!("Ubuntu Mono", "ubuntu-mono"),
    bundled!("Inconsolata", "inconsolata"),
    bundled!("Fira Mono", "fira-mono"),
    bundled!("Noto Sans Mono", "noto-sans-mono"),
    bundled!("Red Hat Mono", "red-hat-mono"),
    bundled!("Overpass Mono", "overpass-mono"),
    bundled!("Geist Mono", "geist-mono"),
    bundled!("Atkinson Hyperlegible Mono", "atkinson-hyperlegible-mono"),
    bundled!("DM Mono", "dm-mono"),
    bundled!("Space Mono", "space-mono"),
    bundled!("Courier Prime", "courier-prime"),
    bundled!("Victor Mono", "victor-mono"),
    bundled!("Martian Mono", "martian-mono"),
    bundled!("Anonymous Pro", "anonymous-pro"),
    bundled!("PT Mono", "pt-mono"),
    bundled!("Cousine", "cousine"),
];

#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    Bundled(&'static str),
    /// A font file, and which face in it (a `.ttc` collection holds several).
    File(PathBuf, u32),
}

impl Source {
    /// Each font's data and face index, the first one the main font.
    pub fn load(&self) -> Result<Vec<(Vec<u8>, u32)>, String> {
        match self {
            Source::Bundled(name) => Ok(bundled(name)
                .expect("resolved names are bundled")
                .files
                .iter()
                .map(|f| (f.to_vec(), 0))
                .collect()),
            Source::File(path, index) => std::fs::read(path)
                .map(|d| vec![(d, *index)])
                .map_err(|e| format!("{}: {e}", path.display())),
        }
    }
}

fn bundled(name: &str) -> Option<&'static Bundled> {
    let want = normalize(name);
    BUNDLED.iter().find(|b| normalize(b.name) == want)
}

pub fn resolve(spec: &str, fonts_dir: &Path) -> Result<Source, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(fc_match("monospace")
            .map(|m| Source::File(m.path, m.index))
            .unwrap_or(Source::Bundled(BUNDLED[0].name)));
    }
    let expanded = match spec.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(rest))
            .unwrap_or_else(|| PathBuf::from(spec)),
        None => PathBuf::from(spec),
    };
    if expanded.is_file() {
        return Ok(Source::File(expanded, 0));
    }
    if let Some(p) = in_fonts_dir(spec, fonts_dir) {
        return Ok(Source::File(p, 0));
    }
    if let Some(b) = bundled(spec) {
        return Ok(Source::Bundled(b.name));
    }
    // A fontconfig pattern (`Family:style=Bold`) names its family first.
    let family = spec.split(':').next().unwrap_or("").trim();
    match fc_match(spec) {
        // fc-match always answers with *something*; only accept it when it
        // is the family that was asked for.
        Some(m) if family.is_empty() || family_matches(family, &m.families) => {
            Ok(Source::File(m.path, m.index))
        }
        Some(m) => Err(format!(
            "font `{spec}` not found (the closest installed is {})",
            m.families.split(',').next().unwrap_or("another font")
        )),
        None => Err(format!("font `{spec}` not found")),
    }
}

/// A file in the fonts dir named `spec`, or `spec` plus a font extension,
/// ignoring case (`MyFont` finds `myfont.TTF`).
fn in_fonts_dir(spec: &str, fonts_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(fonts_dir).ok()?;
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    // An exact name first, then by extension order.
    paths.sort();
    let name_is = |p: &Path, want: &str| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(want))
    };
    paths
        .iter()
        .find(|p| name_is(p, spec))
        .or_else(|| {
            EXTENSIONS
                .iter()
                .find_map(|e| paths.iter().find(|p| name_is(p, &format!("{spec}.{e}"))))
        })
        .filter(|p| p.is_file())
        .cloned()
}

/// Names to offer in the `:font` palette: bundled fonts, files in the fonts
/// dir that can draw text, and installed monospace families that cover
/// English (which leaves out emoji, symbol and icon fonts). Files left out
/// are logged.
pub fn list(fonts_dir: &Path) -> Vec<String> {
    let mut names: BTreeSet<String> = BUNDLED.iter().map(|b| b.name.to_string()).collect();
    if let Ok(entries) = std::fs::read_dir(fonts_dir) {
        for e in entries.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            {
                match usable(&p) {
                    Ok(()) => {
                        names.insert(stem.to_string());
                    }
                    Err(e) => crate::errlog::log(
                        crate::errlog::Level::Warn,
                        "font",
                        format!("not offering {}: {e}", p.display()),
                    ),
                }
            }
        }
    }
    if let Some(out) = run("fc-list", &[":spacing=mono:lang=en", "family"]) {
        for line in out.lines() {
            if let Some(family) = line.split(',').next().map(str::trim)
                && !family.is_empty()
            {
                names.insert(family.to_string());
            }
        }
    }
    names.into_iter().collect()
}

struct Match {
    path: PathBuf,
    index: u32,
    families: String,
}

/// Whether the font file parses and has the Latin letters (its first face).
pub fn usable(path: &Path) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())?;
    match ('a'..='z').all(|c| font.lookup_glyph_index(c) != 0) {
        true => Ok(()),
        false => Err("no Latin letters".into()),
    }
}

fn fc_match(pattern: &str) -> Option<Match> {
    let out = run(
        "fc-match",
        &[pattern, "--format=%{file}\n%{index}\n%{family}"],
    )?;
    let mut lines = out.lines();
    let path = PathBuf::from(lines.next()?.trim());
    let index = lines
        .next()
        .and_then(|i| i.trim().parse().ok())
        .unwrap_or(0);
    let families = lines.next().unwrap_or("").to_string();
    path.is_file().then_some(Match {
        path,
        index,
        families,
    })
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn family_matches(spec: &str, families: &str) -> bool {
    let want = normalize(spec);
    families.split(',').any(|f| normalize(f) == want)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_match_ignores_case_and_spacing() {
        let fams = "JetBrainsMono Nerd Font,JetBrainsMono NF";
        assert!(family_matches("jetbrainsmono nerd font", fams));
        assert!(family_matches("JetBrainsMono-NF", fams));
        assert!(!family_matches("Fira Code", fams));
    }

    #[test]
    fn bundled_fonts_resolve_and_parse() {
        let dir = Path::new("/nonexistent");
        assert_eq!(resolve("fira code", dir), Ok(Source::Bundled("Fira Code")));
        assert!(list(dir).contains(&"Cascadia Code".to_string()));
        for b in BUNDLED {
            let fonts: Vec<fontdue::Font> = b
                .files
                .iter()
                .map(|d| fontdue::Font::from_bytes(*d, Default::default()).unwrap())
                .collect();
            // Between them the subsets cover plain and extended Latin.
            for ch in ['a', 'Z', '?', 'ł', 'ő'] {
                assert!(
                    fonts.iter().any(|f| f.lookup_glyph_index(ch) != 0),
                    "{} lacks {ch}",
                    b.name
                );
            }
        }
    }

    #[test]
    fn fonts_dir_files_resolve_by_stem() {
        let dir = std::env::temp_dir().join(format!("ttyp-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("MyFont.otf");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(resolve("MyFont", &dir), Ok(Source::File(file.clone(), 0)));
        assert_eq!(resolve("myfont", &dir), Ok(Source::File(file.clone(), 0)));
        assert_eq!(
            resolve(file.to_str().unwrap(), &dir),
            Ok(Source::File(file, 0))
        );
        assert!(
            !list(&dir).contains(&"MyFont".to_string()),
            "not a font: not offered"
        );
        let real = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/fira-code-latin.ttf"
        );
        std::fs::copy(real, dir.join("Good.ttf")).unwrap();
        let ext = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/fira-code-latin-ext.ttf"
        );
        std::fs::copy(ext, dir.join("NoLetters.ttf")).unwrap();
        let names = list(&dir);
        assert!(names.contains(&"Good".to_string()));
        assert!(!names.contains(&"NoLetters".to_string()));
        let upper = dir.join("Other.TTF");
        std::fs::write(&upper, b"x").unwrap();
        assert_eq!(resolve("Other", &dir), Ok(Source::File(upper, 0)));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
