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
];

#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    Bundled(&'static str),
    File(PathBuf),
}

impl Source {
    pub fn load(&self) -> Result<Vec<Vec<u8>>, String> {
        match self {
            Source::Bundled(name) => Ok(bundled(name)
                .expect("resolved names are bundled")
                .files
                .iter()
                .map(|f| f.to_vec())
                .collect()),
            Source::File(path) => std::fs::read(path)
                .map(|d| vec![d])
                .map_err(|e| format!("{}: {e}", path.display())),
        }
    }
}

fn bundled(name: &str) -> Option<&'static Bundled> {
    let want = normalize(name);
    BUNDLED.iter().find(|b| normalize(b.name) == want)
}

pub fn resolve(spec: &str, fonts_dir: &Path) -> Result<Source, String> {
    resolve_path(spec, fonts_dir).map(|p| match p {
        Ok(path) => Source::File(path),
        Err(name) => Source::Bundled(name),
    })
}

/// `Ok(path)` for a font file, `Err(name)` for a bundled font.
fn resolve_path(spec: &str, fonts_dir: &Path) -> Result<Result<PathBuf, &'static str>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(fc_match("monospace")
            .map(|(path, _)| path)
            .ok_or(BUNDLED[0].name));
    }
    let expanded = match spec.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(rest))
            .unwrap_or_else(|| PathBuf::from(spec)),
        None => PathBuf::from(spec),
    };
    if expanded.is_file() {
        return Ok(Ok(expanded));
    }
    if let Some(p) = std::iter::once(fonts_dir.join(spec))
        .chain(
            EXTENSIONS
                .iter()
                .map(|e| fonts_dir.join(format!("{spec}.{e}"))),
        )
        .find(|p| p.is_file())
    {
        return Ok(Ok(p));
    }
    if let Some(b) = bundled(spec) {
        return Ok(Err(b.name));
    }
    match fc_match(spec) {
        // fc-match always answers with *something*; only accept it when it
        // is the family that was asked for (or the spec is a full pattern).
        Some((path, families)) if spec.contains(':') || family_matches(spec, &families) => {
            Ok(Ok(path))
        }
        _ => Err(format!("font `{spec}` not found")),
    }
}

/// Names to offer in the `:font` palette: bundled fonts, files in the fonts
/// dir, and installed monospace families.
pub fn list(fonts_dir: &Path) -> Vec<String> {
    let mut names: BTreeSet<String> = BUNDLED.iter().map(|b| b.name.to_string()).collect();
    if let Ok(entries) = std::fs::read_dir(fonts_dir) {
        for e in entries.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            {
                names.insert(stem.to_string());
            }
        }
    }
    if let Some(out) = run("fc-list", &[":spacing=mono", "family"]) {
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

fn fc_match(pattern: &str) -> Option<(PathBuf, String)> {
    let out = run("fc-match", &[pattern, "--format=%{file}\n%{family}"])?;
    let mut lines = out.lines();
    let path = PathBuf::from(lines.next()?.trim());
    let families = lines.next().unwrap_or("").to_string();
    path.is_file().then_some((path, families))
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
        assert_eq!(resolve("MyFont", &dir), Ok(Source::File(file.clone())));
        assert_eq!(
            resolve(file.to_str().unwrap(), &dir),
            Ok(Source::File(file))
        );
        assert!(list(&dir).contains(&"MyFont".to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
