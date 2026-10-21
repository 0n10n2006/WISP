//! Crash recovery: panics are written (redacted) to a log file, and the next
//! start tells the user once that Wisp closed unexpectedly.

use std::fs;
use std::path::{Path, PathBuf};

pub fn install(log_dir: PathBuf, version: String) {
    std::panic::set_hook(Box::new(move |info| {
        let _ = fs::create_dir_all(&log_dir);
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let body = format!(
            "Wisp {version} crashed at {}\nOS: {} {}\n\n{}\n",
            chrono::Local::now().to_rfc3339(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            wisp_core::redact::redact(&info.to_string()),
        );
        let _ = fs::write(log_dir.join(format!("crash-{stamp}.log")), body);
    }));
}

/// Report crash logs not seen before, marking them as seen.
pub fn unseen_crashes(log_dir: &Path) -> Option<String> {
    let entries = fs::read_dir(log_dir).ok()?;
    let mut newest: Option<PathBuf> = None;
    for e in entries.flatten() {
        let p = e.path();
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_new =
            name.starts_with("crash-") && name.ends_with(".log") && !name.ends_with(".seen.log");
        if is_new {
            let seen = p.with_extension("seen.log");
            let _ = fs::rename(&p, &seen);
            if newest.as_ref().is_none_or(|n| seen > *n) {
                newest = Some(seen);
            }
        }
    }
    newest.map(|p| {
        format!(
            "Wisp closed unexpectedly last time. Your data is safe. A crash report was saved to {} — it contains no prompts or keys.",
            p.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_each_crash_once() {
        let dir = tempfile::tempdir().unwrap();
        assert!(unseen_crashes(dir.path()).is_none());
        fs::write(dir.path().join("crash-20261021-120000.log"), "boom").unwrap();
        let msg = unseen_crashes(dir.path()).unwrap();
        assert!(msg.contains("closed unexpectedly"));
        assert!(msg.contains("crash-20261021-120000.seen.log"));
        // Already seen: not reported again.
        assert!(unseen_crashes(dir.path()).is_none());
        assert!(unseen_crashes(&dir.path().join("missing")).is_none());
    }
}
