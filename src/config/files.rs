//! Writing and reading ttyp's own files safely. Every settings or state
//! file is written through `write_atomic` (a sibling temp file, synced, then
//! renamed over the old one), so a full disk or a crash mid-write leaves the
//! old file rather than a truncated one. A state file that can't be read
//! back is moved aside to `<file>.bad` (`read_state`) instead of being
//! overwritten by the next save.

use std::fmt::Display;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Replace `path` with `bytes` all at once. A symlink is followed, so the
/// file it points at is replaced and the link stays. An existing file
/// keeps its permissions.
pub fn write_atomic(path: &Path, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    write_with(path, bytes.as_ref(), None)
}

/// `write_atomic` for a secret: the file is owner-only (0600 on unix),
/// whatever the old one was.
pub fn write_private(path: &Path, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    write_with(path, bytes.as_ref(), Some(0o600))
}

fn write_with(path: &Path, bytes: &[u8], mode: Option<u32>) -> io::Result<()> {
    // Write where a symlink points, so dotfile links survive.
    let path = match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    fs::create_dir_all(&dir)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other(format!("{}: not a file name", path.display())))?;
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let written = (|| {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(m);
        }
        let mut f = opts.open(&tmp)?;
        match mode {
            // A leftover temp file keeps its old mode; force it.
            #[cfg(unix)]
            Some(m) => {
                use std::os::unix::fs::PermissionsExt;
                f.set_permissions(fs::Permissions::from_mode(m))?;
            }
            _ => {
                if let Ok(old) = fs::metadata(&path) {
                    f.set_permissions(old.permissions())?;
                }
            }
        }
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, &path)
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // So the rename itself survives a crash.
    #[cfg(unix)]
    if let Ok(d) = File::open(&dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Where `move_aside` puts `path`: `<file>.bad` next to it.
pub fn bad_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bad");
    path.with_file_name(name)
}

/// Move a file ttyp can't read to `<file>.bad` (replacing an older one),
/// so it can be looked at or fixed by hand.
pub fn move_aside(path: &Path) -> io::Result<PathBuf> {
    let bad = bad_path(path);
    fs::rename(path, &bad)?;
    Ok(bad)
}

/// Read and parse a state file. `Ok(None)` when there is none. A file
/// that can't be read or parsed is moved aside (`move_aside`) and the
/// `Err` says what happened, for a notice or the log; the caller starts
/// empty, and the next save no longer loses the old data.
pub fn read_state<T, E: Display>(
    path: &Path,
    parse: impl FnOnce(&str) -> Result<T, E>,
) -> Result<Option<T>, String> {
    let why = match fs::read_to_string(path) {
        Ok(text) => match parse(&text) {
            Ok(v) => return Ok(Some(v)),
            Err(e) => e.to_string(),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => e.to_string(),
    };
    Err(match move_aside(path) {
        Ok(bad) => format!(
            "could not read {} ({why}); kept it as {}",
            path.display(),
            bad.display()
        ),
        Err(e) => format!(
            "could not read {} ({why}); could not move it aside: {e}",
            path.display()
        ),
    })
}

/// `read_state` for a file whose loss isn't worth a notice: a bad file is
/// logged under `context` and reads as `None`.
pub fn load_state<T, E: Display>(
    context: &str,
    path: &Path,
    parse: impl FnOnce(&str) -> Result<T, E>,
) -> Option<T> {
    read_state(path, parse)
        .map_err(|e| crate::errlog::warn(context, e))
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ttyp-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn write_atomic_creates_and_replaces_without_leftovers() {
        let d = scratch("atomic");
        let p = d.join("sub/state.json");
        write_atomic(&p, "one").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "one");
        write_atomic(&p, b"two, longer").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "two, longer");
        write_atomic(&p, "3").unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "3",
            "no tail of the old file"
        );
        assert!(leftovers(&d.join("sub")).is_empty());
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn a_failed_write_cleans_up_its_temp_file() {
        let d = scratch("atomic-fail");
        fs::create_dir_all(&d).unwrap();
        // A directory in the way of the rename: the write fails, the temp
        // file is cleaned up.
        let p = d.join("taken");
        fs::create_dir_all(p.join("inside")).unwrap();
        assert!(write_atomic(&p, "x").is_err());
        assert!(p.join("inside").is_dir());
        assert!(leftovers(&d).is_empty());
        fs::remove_dir_all(d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_keeps_symlinks_and_modes() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("atomic-link");
        fs::create_dir_all(d.join("dotfiles")).unwrap();
        let real = d.join("dotfiles/config.toml");
        fs::write(&real, "old").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
        let link = d.join("config.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_atomic(&link, "new").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);

        let secret = d.join("token");
        fs::write(&secret, "old").unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&secret, "abc").unwrap();
        let mode = fs::metadata(&secret).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn a_bad_state_file_is_moved_aside() {
        let d = scratch("bad");
        fs::create_dir_all(&d).unwrap();
        let p = d.join("missed_words.json");
        let parse = |t: &str| serde_json::from_str::<Vec<u32>>(t);

        assert_eq!(read_state(&p, parse), Ok(None), "missing is not an error");
        fs::write(&p, "[1, 2]").unwrap();
        assert_eq!(read_state(&p, parse), Ok(Some(vec![1, 2])));

        fs::write(&p, "[1, 2").unwrap();
        let e = read_state(&p, parse).unwrap_err();
        assert!(e.contains("missed_words.json.bad"), "{e}");
        assert!(!p.exists());
        assert_eq!(
            fs::read_to_string(d.join("missed_words.json.bad")).unwrap(),
            "[1, 2"
        );
        // Now it starts empty and the next save doesn't touch the bad copy.
        assert_eq!(load_state("t", &p, parse), None);
        write_atomic(&p, "[]").unwrap();
        assert_eq!(fs::read_to_string(bad_path(&p)).unwrap(), "[1, 2");

        // Not UTF-8 counts as unreadable too.
        fs::write(&p, [0xff, 0xfe]).unwrap();
        assert_eq!(load_state("t", &p, parse), None);
        assert_eq!(fs::read(bad_path(&p)).unwrap(), [0xff, 0xfe]);
        fs::remove_dir_all(d).unwrap();
    }
}
