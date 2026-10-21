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
