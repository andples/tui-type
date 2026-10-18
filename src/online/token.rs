//! The ttyp server token on disk: one line in the data dir, owner-only.

use std::fs;
use std::io;
use std::path::Path;

/// The saved token: `Ok(None)` when there is none (logged out). A token
/// file that exists but can't be read is an error, not a logout.
pub fn load(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s.trim().to_string()).filter(|s| !s.is_empty())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!(
            "could not read the login token {}: {e}",
            path.display()
        )),
    }
}

/// Owner-only (0600), replaced all at once.
pub fn save(path: &Path, token: &str) -> io::Result<()> {
    crate::config::write_private(path, format!("{token}\n"))
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
        assert_eq!(load(&path), Ok(None));
        save(&path, "abc").unwrap();
        assert_eq!(load(&path), Ok(Some("abc".into())));
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
        assert_eq!(load(&path), Ok(None));
        let _ = fs::remove_dir_all(dir);
    }
}
