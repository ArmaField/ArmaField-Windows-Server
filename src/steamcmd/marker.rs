use std::path::Path;
use std::time::{Duration, SystemTime};

/// Decide whether to run `steamcmd validate` now.
///
/// On any filesystem error (missing paths, unreadable metadata, clock
/// regression) err on the safe side and validate - a redundant validate
/// is cheap, skipping a needed one ships a stale server.
pub fn should_validate(
    binary: &Path,
    marker: &Path,
    interval: Duration,
    skip_install: bool,
) -> bool {
    if skip_install {
        return false;
    }
    if !path_exists_safe(binary) {
        return true;
    }
    if !path_exists_safe(marker) {
        return true;
    }
    match marker.metadata().and_then(|m| m.modified()) {
        Ok(mtime) => match SystemTime::now().duration_since(mtime) {
            Ok(age) => age > interval,
            Err(_) => true, // clock went backwards - be safe
        },
        Err(_) => true,
    }
}

fn path_exists_safe(p: &Path) -> bool {
    p.try_exists().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(p: &Path) {
        fs::write(p, b"").unwrap();
    }

    fn set_old_mtime(p: &Path, ago: Duration) {
        let target = SystemTime::now() - ago;
        filetime::set_file_mtime(p, filetime::FileTime::from_system_time(target)).unwrap();
    }

    #[test]
    fn skip_install_always_false() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("srv");
        let marker = dir.path().join("m");
        touch(&bin);
        touch(&marker);
        assert!(!should_validate(
            &bin,
            &marker,
            Duration::from_secs(3600),
            true
        ));
    }

    #[test]
    fn binary_missing_returns_true() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("srv");
        let marker = dir.path().join("m");
        touch(&marker);
        assert!(should_validate(
            &bin,
            &marker,
            Duration::from_secs(3600),
            false
        ));
    }

    #[test]
    fn marker_missing_returns_true() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("srv");
        let marker = dir.path().join("m");
        touch(&bin);
        assert!(should_validate(
            &bin,
            &marker,
            Duration::from_secs(3600),
            false
        ));
    }

    #[test]
    fn fresh_marker_returns_false() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("srv");
        let marker = dir.path().join("m");
        touch(&bin);
        touch(&marker);
        assert!(!should_validate(
            &bin,
            &marker,
            Duration::from_secs(3600),
            false
        ));
    }

    #[test]
    fn stale_marker_returns_true() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("srv");
        let marker = dir.path().join("m");
        touch(&bin);
        touch(&marker);
        set_old_mtime(&marker, Duration::from_secs(90 * 60));
        assert!(should_validate(
            &bin,
            &marker,
            Duration::from_secs(60 * 60),
            false
        ));
    }

    #[test]
    fn zero_interval_always_validates() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("srv");
        let marker = dir.path().join("m");
        touch(&bin);
        touch(&marker);
        assert!(should_validate(&bin, &marker, Duration::ZERO, false));
    }
}
