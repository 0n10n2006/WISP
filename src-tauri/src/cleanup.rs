//! Files Wisp leaves next to its database: migration backups, set-aside
//! settings, kept dictation audio and crash logs. "Delete everything" removes
//! them too, and kept audio follows the history retention period.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

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

/// Delete kept recordings older than `days` (0 deletes them all).
pub fn prune_recordings(data_dir: &Path, days: u32) -> usize {
    let cutoff = SystemTime::now() - Duration::from_secs(u64::from(days) * 86_400);
    let Ok(entries) = fs::read_dir(data_dir.join("recordings")) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            days == 0
                || e.metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t < cutoff)
        })
        .filter(|e| fs::remove_file(e.path()).is_ok())
        .count()
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
    fn prunes_recordings() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("recordings")).unwrap();
        fs::write(dir.path().join("recordings").join("new.webm"), "x").unwrap();
        assert_eq!(prune_recordings(dir.path(), 30), 0);
        assert_eq!(prune_recordings(dir.path(), 0), 1);
    }
}
