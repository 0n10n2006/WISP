//! Browser tabs, history and bookmarks for the bar. History is read from
//! copies of the browsers' own files when the bar asks (at most once a
//! minute) and kept in memory only; tabs are read from the browser windows
//! with UI Automation. Nothing here is stored or sent anywhere.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wisp_core::browsers::{self, Page};

/// History is read again when it's older than this.
const FRESH_FOR: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: i64,
    pub title: String,
    pub browser: String,
    #[serde(skip)]
    pub window: i64,
}

#[derive(Default)]
pub struct Browsers {
    pages: Mutex<Option<(Instant, Arc<Vec<Page>>)>>,
    tabs: Mutex<Vec<Tab>>,
}

fn profiles() -> Vec<browsers::Profile> {
    let (Some(local), Some(roaming)) = (
        std::env::var_os("LOCALAPPDATA"),
        std::env::var_os("APPDATA"),
    ) else {
        return Vec::new();
    };
    browsers::find_profiles(local.as_ref(), roaming.as_ref())
}

/// Names of the browsers with a profile on this PC ("Chrome", "Firefox").
pub fn installed() -> Vec<String> {
    let mut names: Vec<String> = profiles().into_iter().map(|p| p.browser).collect();
    names.dedup();
    names
}

impl Browsers {
    /// History and bookmarks of every profile, read again if stale.
    fn pages(&self) -> Arc<Vec<Page>> {
        if let Some((at, pages)) = self.pages.lock().expect("pages").as_ref() {
            if at.elapsed() < FRESH_FOR {
                return pages.clone();
            }
        }
        let tmp = std::env::temp_dir().join(format!("wisp-browsers-{}", std::process::id()));
        let mut all = Vec::new();
        for p in profiles() {
            all.extend(browsers::read_profile(&p, &tmp));
        }
        let _ = std::fs::remove_dir_all(&tmp);
        let pages = Arc::new(browsers::merge(all));
        *self.pages.lock().expect("pages") = Some((Instant::now(), pages.clone()));
        pages
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Page> {
        let now = chrono::Utc::now().timestamp_millis();
        browsers::search(&self.pages(), query, limit, now)
    }

    /// A page from the last search's data, by id.
    pub fn page(&self, id: i64) -> Option<Page> {
        let pages = self.pages.lock().expect("pages").as_ref()?.1.clone();
        pages.iter().find(|p| p.id == id).cloned()
    }

    /// Forget what was read (when browser search is turned off).
    pub fn forget(&self) {
        *self.pages.lock().expect("pages") = None;
        self.tabs.lock().expect("tabs").clear();
    }

    pub fn set_tabs(&self, tabs: Vec<Tab>) {
        *self.tabs.lock().expect("tabs") = tabs;
    }

    pub fn tab(&self, id: i64) -> Option<Tab> {
        self.tabs
            .lock()
            .expect("tabs")
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }
}

/// A stable id for a tab: its window, place in the strip and title.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn tab_id(window: i64, index: usize, title: &str) -> i64 {
    browsers::page_id(&format!("{window}:{index}:{title}"))
}

#[cfg(windows)]
mod uia {
    use super::{tab_id, Tab};
    use windows::core::Interface;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationSelectionItemPattern,
        IUIAutomationTreeWalker, UIA_DocumentControlTypeId, UIA_SelectionItemPatternId,
        UIA_TabItemControlTypeId,
    };

    /// Elements visited per window, at most (the tab strip is near the top).
    const MAX_NODES: usize = 1500;
    const MAX_DEPTH: usize = 12;

    fn automation() -> Option<IUIAutomation> {
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() }
    }

    /// Tab items in a browser window, in order. Web page content is never
    /// walked (documents are skipped), so pages aren't read.
    fn tab_items(
        auto: &IUIAutomation,
        walker: &IUIAutomationTreeWalker,
        hwnd: i64,
    ) -> Vec<(String, IUIAutomationElement)> {
        let mut out = Vec::new();
        let Ok(root) = (unsafe { auto.ElementFromHandle(HWND(hwnd as isize as *mut _)) }) else {
            return out;
        };
        let mut queue = std::collections::VecDeque::from([(root, 0usize)]);
        let mut seen = 0;
        while let Some((el, depth)) = queue.pop_front() {
            seen += 1;
            if seen > MAX_NODES {
                break;
            }
            let ty = unsafe { el.CurrentControlType() }.unwrap_or_default();
            if ty == UIA_DocumentControlTypeId {
                continue;
            }
            if ty == UIA_TabItemControlTypeId {
                let name = unsafe { el.CurrentName() }
                    .map(|b| b.to_string())
                    .unwrap_or_default();
                out.push((name, el));
                continue;
            }
            if depth >= super::uia::MAX_DEPTH {
                continue;
            }
            let mut child = unsafe { walker.GetFirstChildElement(&el) }.ok();
            while let Some(c) = child {
                let next = unsafe { walker.GetNextSiblingElement(&c) }.ok();
                queue.push_back((c, depth + 1));
                child = next;
            }
        }
        out
    }

    pub fn list(windows: &[(i64, &'static str)]) -> Vec<Tab> {
        let Some(auto) = automation() else {
            return Vec::new();
        };
        let Ok(walker) = (unsafe { auto.ControlViewWalker() }) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (hwnd, browser) in windows {
            for (i, (title, _)) in tab_items(&auto, &walker, *hwnd).into_iter().enumerate() {
                let title: String = title.chars().take(300).collect();
                out.push(Tab {
                    id: tab_id(*hwnd, i, &title),
                    title,
                    browser: browser.to_string(),
                    window: *hwnd,
                });
            }
        }
        out
    }

    /// Select the tab with this title in `hwnd` (the window is already in front).
    pub fn select(hwnd: i64, title: &str) -> bool {
        let Some(auto) = automation() else {
            return false;
        };
        let Ok(walker) = (unsafe { auto.ControlViewWalker() }) else {
            return false;
        };
        let Some((_, el)) = tab_items(&auto, &walker, hwnd)
            .into_iter()
            .find(|(t, _)| t.chars().take(300).collect::<String>() == title)
        else {
            return false;
        };
        unsafe {
            el.GetCurrentPattern(UIA_SelectionItemPatternId)
                .and_then(|p| p.cast::<IUIAutomationSelectionItemPattern>())
                .and_then(|p| p.Select())
                .is_ok()
        }
    }
}

#[cfg(not(windows))]
mod uia {
    use super::Tab;
    pub fn list(_windows: &[(i64, &'static str)]) -> Vec<Tab> {
        Vec::new()
    }
    pub fn select(_hwnd: i64, _title: &str) -> bool {
        false
    }
}

/// Tabs of every open browser window (run on the COM worker).
pub fn list_tabs() -> Vec<Tab> {
    let windows: Vec<(i64, &'static str)> = crate::desktop::open_windows()
        .into_iter()
        .filter_map(|w| browsers::browser_for_process(&w.app).map(|b| (w.id, b)))
        .collect();
    uia::list(&windows)
}

/// Bring a tab's window to the front and select the tab.
pub fn switch_to(tab: &Tab) -> bool {
    crate::desktop::focus_window(tab.window) && uia::select(tab.window, &tab.title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_ids_are_stable_and_distinct() {
        assert_eq!(tab_id(1, 0, "Inbox"), tab_id(1, 0, "Inbox"));
        assert_ne!(tab_id(1, 0, "Inbox"), tab_id(1, 1, "Inbox"));
        let b = Browsers::default();
        b.set_tabs(vec![Tab {
            id: 5,
            title: "Inbox".into(),
            browser: "Chrome".into(),
            window: 9,
        }]);
        assert_eq!(b.tab(5).unwrap().window, 9);
        assert!(b.tab(6).is_none());
        assert!(b.page(1).is_none());
        b.forget();
        assert!(b.tab(5).is_none());
    }
}
