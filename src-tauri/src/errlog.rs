//! A small local error log (`<appdata>/logs/errors.log`) so problems that
//! would otherwise be silent — a failed command, a script error in a window —
//! leave a trace the user can see in Settings → Data and send to us. Lines are
//! redacted and capped; nothing is sent anywhere.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LINES: usize = 200;
const MAX_LINE_CHARS: usize = 600;
/// At most this many entries per minute (a looping error can't fill the disk).
const PER_MINUTE: u64 = 30;

static PATH: OnceLock<PathBuf> = OnceLock::new();
static WRITE: Mutex<()> = Mutex::new(());
static WINDOW: AtomicU64 = AtomicU64::new(0);
static COUNT: AtomicU64 = AtomicU64::new(0);

/// Where the log lives; call once at start-up.
pub fn init(log_dir: &Path) {
    let _ = PATH.set(log_dir.join("errors.log"));
}

pub fn path() -> Option<&'static Path> {
    PATH.get().map(PathBuf::as_path)
}

fn allowed() -> bool {
    let minute = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 60)
        .unwrap_or(0);
    if WINDOW.swap(minute, Ordering::SeqCst) != minute {
        COUNT.store(0, Ordering::SeqCst);
    }
    COUNT.fetch_add(1, Ordering::SeqCst) < PER_MINUTE
}

/// One log line: time, where it happened, and the redacted message.
pub fn format_line(source: &str, message: &str) -> String {
    let clean: String = wisp_core::redact::redact(message)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_LINE_CHARS)
        .collect();
    let source: String = source
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
        .take(40)
        .collect();
    format!(
        "{} [{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        source,
        clean.trim()
    )
}

/// Append to the log, keeping only the newest `MAX_LINES` lines.
pub fn append_to(path: &Path, line: &str) {
    let _guard = WRITE.lock();
    let mut lines: Vec<String> = fs::read_to_string(path)
        .map(|s| s.lines().map(str::to_owned).collect())
        .unwrap_or_default();
    lines.push(line.to_string());
    let start = lines.len().saturating_sub(MAX_LINES);
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, lines[start..].join("\n") + "\n");
}

/// Record an error from `source` (e.g. "island", "set_mode").
pub fn log(source: &str, message: &str) {
    if message.trim().is_empty() || !allowed() {
        return;
    }
    if let Some(path) = path() {
        append_to(path, &format_line(source, message));
    }
}

/// The newest `n` entries, newest last.
pub fn recent(n: usize) -> Vec<String> {
    let Some(path) = path() else {
        return Vec::new();
    };
    let text = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_redacted_capped_and_trimmed() {
        let line = format_line(
            "island<script>",
            "boom\nkey AIzaSyFAKE00000000000000000000000000000 here",
        );
        assert!(line.contains("[islandscript]"));
        assert!(!line.contains("AIzaSyFAKE"));
        assert!(!line.contains('\n'));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("logs").join("errors.log");
        for i in 0..(MAX_LINES + 5) {
            append_to(&p, &format!("line {i}"));
        }
        let text = fs::read_to_string(&p).unwrap();
        assert_eq!(text.lines().count(), MAX_LINES);
        assert!(text
            .lines()
            .last()
            .unwrap()
            .ends_with(&format!("line {}", MAX_LINES + 4)));
    }
}
