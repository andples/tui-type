//! The ttyp server token on disk: one line in the data dir, owner-only.

use std::fs;
use std::io;
use std::path::Path;

pub fn load(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn save(path: &Path, token: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    // An existing file keeps its old mode; force it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    io::Write::write_all(&mut f, format!("{token}\n").as_bytes())
}

pub fn delete(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_owner_only() {
        let dir = std::env::temp_dir().join(format!("ttyp-token-{}", std::process::id()));
        let path = dir.join("token");
        assert_eq!(load(&path), None);
        save(&path, "abc").unwrap();
        assert_eq!(load(&path).as_deref(), Some("abc"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        delete(&path).unwrap();
        delete(&path).unwrap();
        assert_eq!(load(&path), None);
        let _ = fs::remove_dir_all(dir);
    }
}
