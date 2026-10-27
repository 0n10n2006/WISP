//! Files Wisp leaves next to its database: migration backups, set-aside
//! settings, kept dictation audio and crash logs. "Delete everything" removes
//! them too, and kept audio follows the history retention period.

use std::fs;
use std::path::Path;

fn is_leftover(name: &str) -> bool {
    (name.starts_with("wisp.v") && name.ends_with(".bak"))
        || (name.starts_with("settings.corrupt-") && name.ends_with(".json"))
        || name == "settings.json.tmp"
}

/// Remove backups, set-aside settings, recordings and logs from `data_dir`.
/// Returns how many files were removed.
pub fn wipe_leftovers(data_dir: &Path) -> usize {
    let mut removed = 0;
    if let Ok(entries) = fs::read_dir(data_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_file())
                && is_leftover(&name)
                && fs::remove_file(e.path()).is_ok()
            {
                removed += 1;
            }
        }
    }
    for sub in ["recordings", "logs"] {
        let dir = data_dir.join(sub);
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                if e.file_type().is_ok_and(|t| t.is_file()) && fs::remove_file(e.path()).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    removed
}

/// Remove database backups made before upgrades (they hold old copies of
/// whatever the user is deleting).
pub fn remove_backups(data_dir: &Path) {
    if let Ok(entries) = fs::read_dir(data_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("wisp.v") && name.ends_with(".bak") {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

/// Remove what older versions kept for voice: recordings and the offline
/// speech models (Whisper, up to ~550 MB each). Voice now uses Windows'
/// own voice typing and keeps nothing.
pub fn remove_voice_leftovers(data_dir: &Path) {
    for sub in ["recordings", "models"] {
        let dir = data_dir.join(sub);
        if dir.is_dir() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_wisp_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for f in [
            "wisp.v1.bak",
            "settings.corrupt-20261021.json",
            "wisp.db",
            "settings.json",
            "notes.txt",
        ] {
            fs::write(d.join(f), "x").unwrap();
        }
        for sub in ["recordings", "logs"] {
            fs::create_dir(d.join(sub)).unwrap();
            fs::write(d.join(sub).join("a"), "x").unwrap();
        }
        assert_eq!(wipe_leftovers(d), 4);
        assert!(d.join("wisp.db").exists());
        assert!(d.join("settings.json").exists());
        assert!(d.join("notes.txt").exists());
        assert!(!d.join("wisp.v1.bak").exists());
        assert!(!d.join("recordings").join("a").exists());
    }

    #[test]
    fn removes_old_voice_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("recordings")).unwrap();
        fs::write(dir.path().join("recordings").join("new.webm"), "x").unwrap();
        fs::create_dir(dir.path().join("models")).unwrap();
        fs::write(dir.path().join("models").join("ggml-small.bin"), "x").unwrap();
        fs::write(dir.path().join("settings.json"), "{}").unwrap();
        remove_voice_leftovers(dir.path());
        assert!(!dir.path().join("recordings").exists());
        assert!(!dir.path().join("models").exists());
        assert!(dir.path().join("settings.json").exists());
    }
}
