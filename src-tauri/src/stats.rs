//! Live system stats for the dashboard's Stats tab. Polled by the UI only
//! while that tab is visible; nothing is recorded or sent anywhere.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use sysinfo::{Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessStat {
    pub name: String,
    /// Instances grouped under this name (e.g. browser helpers).
    pub count: u32,
    pub memory_bytes: u64,
    pub cpu: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskStat {
    pub mount: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSnapshot {
    pub cpu: f32,
    pub cores: Vec<f32>,
    pub cpu_name: String,
    pub memory_total: u64,
    pub memory_used: u64,
    pub disks: Vec<DiskStat>,
    /// Bytes per second since the previous snapshot.
    pub net_down: u64,
    pub net_up: u64,
    pub uptime_secs: u64,
    pub top: Vec<ProcessStat>,
}

pub struct Stats {
    inner: Mutex<Option<Inner>>,
}

struct Inner {
    sys: System,
    nets: Networks,
    disks: Disks,
    last: Instant,
    /// Snapshots taken so far; slower-changing data refreshes less often.
    ticks: u64,
    top: Vec<ProcessStat>,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

/// Merge processes by name and keep the `n` heaviest by memory.
pub fn top_processes(
    items: impl IntoIterator<Item = (String, u64, f32)>,
    n: usize,
) -> Vec<ProcessStat> {
    let mut by_name: HashMap<String, ProcessStat> = HashMap::new();
    for (name, mem, cpu) in items {
        let key = name.trim_end_matches(".exe").to_string();
        if key.is_empty() {
            continue;
        }
        let e = by_name.entry(key.clone()).or_insert(ProcessStat {
            name: key,
            count: 0,
            memory_bytes: 0,
            cpu: 0.0,
        });
        e.count += 1;
        e.memory_bytes += mem;
        e.cpu += cpu;
    }
    let mut v: Vec<_> = by_name.into_values().collect();
    v.sort_by(|a, b| {
        b.memory_bytes
            .cmp(&a.memory_bytes)
            .then(a.name.cmp(&b.name))
    });
    v.truncate(n);
    v
}

impl Stats {
    pub fn snapshot(&self) -> StatsSnapshot {
        let mut guard = self.inner.lock().expect("stats");
        let inner = guard.get_or_insert_with(|| {
            let mut sys = System::new();
            sys.refresh_cpu_usage();
            Inner {
                sys,
                nets: Networks::new_with_refreshed_list(),
                disks: Disks::new_with_refreshed_list(),
                last: Instant::now(),
                ticks: 0,
                top: Vec::new(),
            }
        });
        inner.ticks += 1;
        inner.sys.refresh_cpu_usage();
        inner.sys.refresh_memory();
        inner.nets.refresh(true);
        // Walking every process is the expensive part: do it every other
        // snapshot. Disk free space changes slowly: every tenth.
        let refresh_procs = !inner.ticks.is_multiple_of(2);
        if refresh_procs {
            inner.sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().with_memory().with_cpu(),
            );
        }
        if inner.ticks.is_multiple_of(10) {
            inner.disks.refresh(true);
        }
        let elapsed = inner.last.elapsed().as_secs_f64().max(0.2);
        inner.last = Instant::now();
        let (down, up) = inner.nets.iter().fold((0u64, 0u64), |(d, u), (_, n)| {
            (d + n.received(), u + n.transmitted())
        });
        let cores_n = inner.sys.cpus().len().max(1) as f32;
        if refresh_procs {
            inner.top = top_processes(
                inner.sys.processes().values().map(|p| {
                    (
                        p.name().to_string_lossy().into_owned(),
                        p.memory(),
                        p.cpu_usage() / cores_n,
                    )
                }),
                6,
            );
        }
        let top = inner.top.clone();
        let disks = inner
            .disks
            .list()
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| DiskStat {
                mount: d.mount_point().display().to_string(),
                total_bytes: d.total_space(),
                free_bytes: d.available_space(),
            })
            .take(6)
            .collect();
        StatsSnapshot {
            cpu: inner.sys.global_cpu_usage(),
            cores: inner.sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
            cpu_name: inner
                .sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_default(),
            memory_total: inner.sys.total_memory(),
            memory_used: inner.sys.used_memory(),
            disks,
            net_down: (down as f64 / elapsed) as u64,
            net_up: (up as f64 / elapsed) as u64,
            uptime_secs: System::uptime(),
            top,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_processes_by_name() {
        let top = top_processes(
            [
                ("chrome.exe".to_string(), 300, 1.0),
                ("chrome.exe".to_string(), 200, 2.0),
                ("code.exe".to_string(), 400, 0.5),
                ("".to_string(), 9999, 0.0),
                ("tiny".to_string(), 1, 0.0),
            ],
            2,
        );
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].name, "chrome");
        assert_eq!(top[0].count, 2);
        assert_eq!(top[0].memory_bytes, 500);
        assert!((top[0].cpu - 3.0).abs() < f32::EPSILON);
        assert_eq!(top[1].name, "code");
    }

    #[test]
    fn snapshot_reports_this_machine() {
        let s = Stats::default();
        let snap = s.snapshot();
        assert!(snap.memory_total > 0);
        assert!(!snap.cores.is_empty());
        assert!(!snap.top.is_empty());
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub plugged_in: bool,
    /// Estimated time left on battery, when Windows knows it.
    pub secs_left: Option<u32>,
}

/// The battery, or `None` on a desktop PC.
#[cfg(windows)]
pub fn battery() -> Option<Battery> {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s) }.ok()?;
    const NO_BATTERY: u8 = 128;
    const CHARGING: u8 = 8;
    if s.BatteryFlag == 255 || s.BatteryFlag & NO_BATTERY != 0 || s.BatteryLifePercent > 100 {
        return None;
    }
    Some(Battery {
        percent: s.BatteryLifePercent,
        charging: s.BatteryFlag & CHARGING != 0,
        plugged_in: s.ACLineStatus == 1,
        secs_left: (s.BatteryLifeTime != u32::MAX && s.ACLineStatus != 1)
            .then_some(s.BatteryLifeTime),
    })
}

#[cfg(not(windows))]
pub fn battery() -> Option<Battery> {
    None
}

/// This PC's address on the local network. Connecting a UDP socket only
/// picks the outgoing interface; no packet is sent.
pub fn local_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then(|| ip.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcInfo {
    pub battery: Option<Battery>,
    pub local_ip: Option<String>,
    pub stats: StatsSnapshot,
}

/// Programs Wisp never ends: Windows itself, security software, and Wisp.
const PROTECTED: &[&str] = &[
    "system",
    "idle",
    "system idle process",
    "registry",
    "secure system",
    "memory compression",
    "smss",
    "csrss",
    "wininit",
    "winlogon",
    "services",
    "lsass",
    "lsaiso",
    "svchost",
    "fontdrvhost",
    "dwm",
    "sihost",
    "ctfmon",
    "msmpeng",
    "nissrv",
    "securityhealthservice",
    "wisp",
];

/// Process name as shown and compared: "Chrome.exe" → "chrome".
pub fn process_key(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .trim_end_matches(".exe")
        .to_string()
}

pub fn is_protected(name: &str) -> bool {
    PROTECTED.contains(&process_key(name).as_str())
}

/// Running programs, grouped by name, for the bar.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcGroup {
    pub name: String,
    pub count: u32,
    /// The biggest process of the group.
    pub pid: u32,
    pub memory_bytes: u64,
    pub cpu: f32,
    /// One of its windows (for switching to it).
    pub window: Option<i64>,
    pub protected: bool,
}

/// Group `(name, pid, memory, cpu)` by name, heaviest first; `windows` are
/// `(process name, window id)` pairs.
pub fn group_processes(
    items: impl IntoIterator<Item = (String, u32, u64, f32)>,
    windows: &[(String, i64)],
) -> Vec<ProcGroup> {
    let mut by: HashMap<String, (ProcGroup, u64)> = HashMap::new();
    for (name, pid, mem, cpu) in items {
        let shown = name.trim().trim_end_matches(".exe").to_string();
        if shown.is_empty() {
            continue;
        }
        let key = process_key(&shown);
        let e = by.entry(key.clone()).or_insert_with(|| {
            (
                ProcGroup {
                    name: shown.clone(),
                    count: 0,
                    pid,
                    memory_bytes: 0,
                    cpu: 0.0,
                    window: windows.iter().find(|(a, _)| *a == key).map(|(_, w)| *w),
                    protected: is_protected(&key),
                },
                0,
            )
        });
        e.0.count += 1;
        e.0.memory_bytes += mem;
        e.0.cpu += cpu;
        if mem > e.1 {
            e.1 = mem;
            e.0.pid = pid;
        }
    }
    let mut v: Vec<ProcGroup> = by.into_values().map(|(g, _)| g).collect();
    v.sort_by(|a, b| {
        b.memory_bytes
            .cmp(&a.memory_bytes)
            .then(a.name.cmp(&b.name))
    });
    v
}

impl Stats {
    /// Every running program, grouped by name.
    pub fn processes(&self, windows: &[(String, i64)]) -> Vec<ProcGroup> {
        let _ = self.snapshot();
        let mut guard = self.inner.lock().expect("stats");
        let Some(inner) = guard.as_mut() else {
            return Vec::new();
        };
        inner.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );
        let cores = inner.sys.cpus().len().max(1) as f32;
        group_processes(
            inner.sys.processes().iter().map(|(pid, p)| {
                (
                    p.name().to_string_lossy().into_owned(),
                    pid.as_u32(),
                    p.memory(),
                    p.cpu_usage() / cores,
                )
            }),
            windows,
        )
    }
}

/// End every process called `name` (never a protected one, never Wisp,
/// never with more rights than Wisp has). Returns how many were ended.
pub fn end_processes(name: &str) -> wisp_core::Result<usize> {
    let key = process_key(name);
    if key.is_empty() || key.len() > 120 {
        return Err(wisp_core::WispError::invalid("Which program?"));
    }
    if is_protected(&key) {
        return Err(wisp_core::WispError::new(
            wisp_core::ErrorCode::PermissionDenied,
            format!("Wisp won't end {name}: Windows needs it."),
        ));
    }
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let me = std::process::id();
    let targets: Vec<_> = sys
        .processes()
        .iter()
        .filter(|(pid, p)| pid.as_u32() != me && process_key(&p.name().to_string_lossy()) == key)
        .collect();
    if targets.is_empty() {
        return Err(wisp_core::WispError::invalid(format!(
            "{name} isn't running."
        )));
    }
    let ended = targets.iter().filter(|(_, p)| p.kill()).count();
    if ended == 0 {
        return Err(wisp_core::WispError::new(
            wisp_core::ErrorCode::PermissionDenied,
            format!("Windows didn't let Wisp end {name} (it may be running as administrator)."),
        ));
    }
    Ok(ended)
}

#[cfg(test)]
mod process_tests {
    use super::*;

    #[test]
    fn groups_by_name_and_guards_windows_itself() {
        let g = group_processes(
            [
                ("chrome.exe".to_string(), 10, 100, 1.0),
                ("chrome.exe".to_string(), 11, 300, 2.0),
                ("csrss.exe".to_string(), 600, 5, 0.0),
                ("Notepad.exe".to_string(), 20, 50, 0.0),
                (String::new(), 1, 1, 0.0),
            ],
            &[("chrome".to_string(), 77), ("notepad".to_string(), 88)],
        );
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].name, "chrome");
        assert_eq!(
            (g[0].count, g[0].pid, g[0].memory_bytes, g[0].window),
            (2, 11, 400, Some(77))
        );
        assert!(!g[0].protected);
        let csrss = g.iter().find(|p| p.name == "csrss").unwrap();
        assert!(csrss.protected && csrss.window.is_none());
        assert_eq!(
            g.iter().find(|p| p.name == "Notepad").unwrap().window,
            Some(88)
        );
        assert!(is_protected("LSASS.EXE") && is_protected("wisp") && !is_protected("explorer"));
        assert!(end_processes("csrss").is_err());
        assert!(end_processes("Wisp.exe").is_err());
        assert!(end_processes("").is_err());
        assert!(end_processes("no-such-program-wisp-test").is_err());
    }
}
