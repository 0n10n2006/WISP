//! Power actions from the dashboard: lock, sleep, restart, shut down.
//! The UI confirms each one first; restart and shut down also wait 10 s so
//! they can still be cancelled. Only fixed system calls and the fixed
//! `shutdown.exe` arguments below are used.

use serde::{Deserialize, Serialize};
use wisp_core::{ErrorCode, Result, WispError};

#[cfg_attr(not(windows), allow(dead_code))]
pub const SHUTDOWN_DELAY_SECS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerAction {
    Lock,
    Sleep,
    Restart,
    Shutdown,
    /// Abort a pending restart or shut down.
    Cancel,
}

impl PowerAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lock => "Lock",
            Self::Sleep => "Sleep",
            Self::Restart => "Restart",
            Self::Shutdown => "Shut down",
            Self::Cancel => "Cancel restart/shut down",
        }
    }

    /// Fixed `shutdown.exe` arguments, where that tool is used.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn shutdown_args(self) -> Option<Vec<String>> {
        let delay = SHUTDOWN_DELAY_SECS.to_string();
        match self {
            Self::Restart => Some(vec!["/r".into(), "/t".into(), delay]),
            Self::Shutdown => Some(vec!["/s".into(), "/t".into(), delay]),
            Self::Cancel => Some(vec!["/a".into()]),
            _ => None,
        }
    }
}

#[cfg(windows)]
pub fn run(action: PowerAction) -> Result<()> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::System::Power::SetSuspendState;
    use windows::Win32::System::Shutdown::LockWorkStation;
    let failed = |e: String| {
        WispError::new(
            ErrorCode::Internal,
            format!("{} failed: {e}", action.label()),
        )
    };
    match action {
        PowerAction::Lock => unsafe { LockWorkStation() }.map_err(|e| failed(e.to_string())),
        PowerAction::Sleep => {
            if unsafe { SetSuspendState(false, false, false) } {
                Ok(())
            } else {
                Err(failed("Windows refused to sleep".into()))
            }
        }
        _ => {
            let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
            let exe = std::path::PathBuf::from(root)
                .join("System32")
                .join("shutdown.exe");
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let status = std::process::Command::new(exe)
                .args(action.shutdown_args().unwrap_or_default())
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map_err(|e| failed(e.to_string()))?;
            if status.success() || action == PowerAction::Cancel {
                Ok(())
            } else {
                Err(failed(format!("exit code {:?}", status.code())))
            }
        }
    }
}

#[cfg(not(windows))]
pub fn run(_action: PowerAction) -> Result<()> {
    Err(WispError::new(
        ErrorCode::Unsupported,
        "Power actions are only available on Windows.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_arguments_only() {
        assert_eq!(
            PowerAction::Restart.shutdown_args().unwrap(),
            ["/r", "/t", "10"]
        );
        assert_eq!(
            PowerAction::Shutdown.shutdown_args().unwrap(),
            ["/s", "/t", "10"]
        );
        assert_eq!(PowerAction::Cancel.shutdown_args().unwrap(), ["/a"]);
        assert!(PowerAction::Lock.shutdown_args().is_none());
        let a: PowerAction = serde_json::from_str("\"shutdown\"").unwrap();
        assert_eq!(a, PowerAction::Shutdown);
        assert!(serde_json::from_str::<PowerAction>("\"format c:\"").is_err());
    }
}
