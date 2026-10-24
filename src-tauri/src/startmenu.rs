//! Make Wisp easy to find in Windows search: the process uses the same
//! AppUserModelID as its Start-menu shortcut, and when Wisp runs without one
//! (copied or run straight from a download) it adds a per-user "Wisp"
//! shortcut, which Windows Search picks up within seconds.

/// Identifier shared by the process, its shortcut and its notifications.
#[cfg_attr(not(windows), allow(dead_code))]
pub const APP_ID: &str = "app.wisp.desktop";
#[cfg_attr(not(windows), allow(dead_code))]
const DESCRIPTION: &str = "Wisp – AI companion for Windows";

/// Where a Start-menu shortcut called "Wisp" would be, in each Programs
/// folder (per-user first, then all users).
#[cfg_attr(not(windows), allow(dead_code))]
fn shortcut_paths(programs: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    programs
        .iter()
        .flat_map(|p| [p.join("Wisp.lnk"), p.join("Wisp").join("Wisp.lnk")])
        .collect()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::path::PathBuf;
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{
        FOLDERID_CommonPrograms, FOLDERID_Programs, IShellLinkW, SHGetKnownFolderPath,
        SetCurrentProcessExplicitAppUserModelID, ShellLink, KF_FLAG_DEFAULT,
    };

    /// PKEY_AppUserModel_ID.
    const PKEY_APP_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };

    pub fn set_app_id() {
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID));
        }
    }

    fn known(id: &windows::core::GUID) -> Option<PathBuf> {
        unsafe {
            let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
            let s = p.to_string().ok();
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
            s.map(PathBuf::from)
        }
    }

    /// Runs on the COM worker.
    pub fn ensure_shortcut() {
        let Some(user) = known(&FOLDERID_Programs) else {
            return;
        };
        let mut programs = vec![user.clone()];
        programs.extend(known(&FOLDERID_CommonPrograms));
        if shortcut_paths(&programs).iter().any(|p| p.exists()) {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let target = user.join("Wisp.lnk");
        if create(&exe, &target).is_ok() {
            crate::selftest::mark("start-menu-shortcut");
        }
    }

    fn create(exe: &std::path::Path, at: &std::path::Path) -> windows::core::Result<()> {
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let exe_w = HSTRING::from(exe.as_os_str());
            link.SetPath(&exe_w)?;
            if let Some(dir) = exe.parent() {
                link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
            }
            link.SetDescription(&HSTRING::from(DESCRIPTION))?;
            link.SetIconLocation(&exe_w, 0)?;
            let store: IPropertyStore = link.cast()?;
            let value = PROPVARIANT::from(APP_ID);
            store.SetValue(&PKEY_APP_ID, &value)?;
            store.Commit()?;
            let file: IPersistFile = link.cast()?;
            file.Save(PCWSTR(HSTRING::from(at.as_os_str()).as_ptr()), true)
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn set_app_id() {}
    pub fn ensure_shortcut() {}
}

pub use imp::{ensure_shortcut, set_app_id};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn looks_for_both_shortcut_layouts() {
        let p = shortcut_paths(&[PathBuf::from("/u"), PathBuf::from("/all")]);
        assert_eq!(
            p,
            vec![
                PathBuf::from("/u/Wisp.lnk"),
                PathBuf::from("/u/Wisp/Wisp.lnk"),
                PathBuf::from("/all/Wisp.lnk"),
                PathBuf::from("/all/Wisp/Wisp.lnk"),
            ]
        );
    }
}
