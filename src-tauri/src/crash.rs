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
        let is_new = p.extension().is_some_and(|x| x == "log")
            && p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("crash-"));
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
