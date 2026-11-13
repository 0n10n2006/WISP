//! Browser tabs, history and bookmarks for the bar. History is read from
//! copies of the browsers' own files when the bar asks (at most once a
//! minute) and kept in memory only; tabs are read from the browser windows
//! with UI Automation. Nothing here is stored or sent anywhere.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use wisp_core::browsers::{self, Indexed, Page};

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

/// Tabs are read again when the list is older than this.
const TABS_FRESH_FOR: Duration = Duration::from_secs(5);

/// What was read from the browsers, and what their files looked like then.
struct Cache {
    at: Instant,
    pages: Arc<Vec<Indexed>>,
    stamp: Vec<Stamp>,
}

/// A history file's path, size and modified time: unchanged means there is
/// nothing new to read.
type Stamp = (PathBuf, u64, Option<SystemTime>);

#[derive(Default)]
pub struct Browsers {
    pages: Mutex<Option<Cache>>,
    /// Held while history is being read, so it's only read once at a time.
    refreshing: Mutex<()>,
    tabs: Mutex<Vec<Tab>>,
    tabs_at: Mutex<Option<Instant>>,
}

/// The files a profile's history lives in (with the write-ahead log, where
/// recent visits sit until the browser folds them in).
fn stamps(profiles: &[browsers::Profile]) -> Vec<Stamp> {
    let mut out = Vec::new();
    for p in profiles {
        let names: &[&str] = match p.engine {
            browsers::Engine::Chromium => &["History", "History-wal", "Bookmarks"],
            browsers::Engine::Firefox => &["places.sqlite", "places.sqlite-wal"],
        };
        for n in names {
            let path = p.dir.join(n);
            let meta = std::fs::metadata(&path).ok();
            out.push((
                path,
                meta.as_ref().map_or(0, |m| m.len()),
                meta.and_then(|m| m.modified().ok()),
            ));
        }
    }
    out
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
    fn pages(&self) -> Arc<Vec<Indexed>> {
        self.pages_with(&profiles(), browsers::read_profile)
    }

    fn cached(&self, fresh_only: bool) -> Option<Arc<Vec<Indexed>>> {
        let guard = self.pages.lock().expect("pages");
        let c = guard.as_ref()?;
        (!fresh_only || c.at.elapsed() < FRESH_FOR).then(|| c.pages.clone())
    }

    /// [`Self::pages`] with the reading injected (for tests). Only one
    /// caller reads at a time; the others use what's already known, and
    /// nothing is read when the browsers' files haven't changed.
    fn pages_with(
        &self,
        profiles: &[browsers::Profile],
        read: impl Fn(&browsers::Profile, &Path) -> Vec<Page>,
    ) -> Arc<Vec<Indexed>> {
        if let Some(pages) = self.cached(true) {
            return pages;
        }
        let _reading = match self.refreshing.try_lock() {
            Ok(g) => g,
            Err(_) => {
                // Someone else is reading: use the older data meanwhile, or
                // wait for them if there is none yet.
                if let Some(stale) = self.cached(false) {
                    return stale;
                }
                let g = self.refreshing.lock().expect("refreshing");
                if let Some(pages) = self.cached(false) {
                    return pages;
                }
                g
            }
        };
        if let Some(pages) = self.cached(true) {
            return pages;
        }
        let stamp = stamps(profiles);
        {
            let mut guard = self.pages.lock().expect("pages");
            if let Some(c) = guard.as_mut().filter(|c| c.stamp == stamp) {
                c.at = Instant::now();
                return c.pages.clone();
            }
        }
        static RUN: AtomicU64 = AtomicU64::new(0);
        let tmp = std::env::temp_dir().join(format!(
            "wisp-browsers-{}-{}",
            std::process::id(),
            RUN.fetch_add(1, Ordering::Relaxed)
        ));
        let mut all = Vec::new();
        for p in profiles {
            all.extend(read(p, &tmp));
        }
        let _ = std::fs::remove_dir_all(&tmp);
        let pages = Arc::new(browsers::index(browsers::merge(all)));
        *self.pages.lock().expect("pages") = Some(Cache {
            at: Instant::now(),
            pages: pages.clone(),
            stamp,
        });
        pages
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Page> {
        let now = chrono::Utc::now().timestamp_millis();
        browsers::search(&self.pages(), query, limit, now)
    }

    /// A page from the last search's data, by id.
    pub fn page(&self, id: i64) -> Option<Page> {
        let pages = self.cached(false)?;
        pages
            .iter()
            .find(|p| p.page.id == id)
            .map(|p| p.page.clone())
    }

    /// Forget what was read (when browser search is turned off).
    pub fn forget(&self) {
        *self.pages.lock().expect("pages") = None;
        self.tabs.lock().expect("tabs").clear();
        *self.tabs_at.lock().expect("tabs at") = None;
    }

    pub fn set_tabs(&self, tabs: Vec<Tab>) {
        *self.tabs.lock().expect("tabs") = tabs;
        *self.tabs_at.lock().expect("tabs at") = Some(Instant::now());
    }

    /// The open tabs: the list read a moment ago, or a new one from `list`.
    pub fn tabs_or(&self, list: impl FnOnce() -> Vec<Tab>) -> Vec<Tab> {
        let fresh = self
            .tabs_at
            .lock()
            .expect("tabs at")
            .is_some_and(|t| t.elapsed() < TABS_FRESH_FOR);
        if !fresh {
            self.set_tabs(list());
        }
        self.tabs.lock().expect("tabs").clone()
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
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationSelectionItemPattern,
        IUIAutomationTreeWalker, UIA_DocumentControlTypeId, UIA_SelectionItemPatternId,
        UIA_TabItemControlTypeId,
    };

    /// Elements visited per window, at most (the tab strip is near the top).
    const MAX_NODES: usize = 1500;
    const MAX_DEPTH: usize = 12;

    fn automation() -> Option<IUIAutomation> {
        unsafe {
            // Whichever thread this runs on needs COM (once is enough).
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
        }
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

/// Tabs of every open browser window (slow: run on a blocking thread).
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

    #[test]
    fn history_is_read_once_however_many_searches_ask() {
        let b = Arc::new(Browsers::default());
        let reads = Arc::new(AtomicU64::new(0));
        let profile = browsers::Profile {
            browser: "Chrome".into(),
            engine: browsers::Engine::Chromium,
            dir: std::env::temp_dir().join("wisp-no-such-profile"),
        };
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let (b, reads, profile) = (b.clone(), reads.clone(), profile.clone());
                std::thread::spawn(move || {
                    b.pages_with(std::slice::from_ref(&profile), |p, _tmp| {
                        reads.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(80));
                        vec![Page {
                            id: 7,
                            title: "GitHub".into(),
                            url: "https://github.com/".into(),
                            browser: p.browser.clone(),
                            kind: "history",
                            visits: 1,
                            last_visit: 0,
                        }]
                    })
                    .len()
                })
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap(), 1);
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(b.page(7).unwrap().title, "GitHub");
    }

    #[test]
    fn tabs_are_listed_at_most_every_few_seconds() {
        let b = Browsers::default();
        let tab = |id| Tab {
            id,
            title: "Inbox".into(),
            browser: "Chrome".into(),
            window: 1,
        };
        assert_eq!(b.tabs_or(|| vec![tab(1)])[0].id, 1);
        // Asked again straight away: the same list, without looking again.
        assert_eq!(b.tabs_or(|| vec![tab(2)])[0].id, 1);
    }
}
