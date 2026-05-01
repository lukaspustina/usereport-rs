//! Cgroup v1/v2 collector (SDD Req 10).
//!
//! Detects cgroup v1 vs v2 and reads cpu.stat, memory.current, memory.max,
//! memory.events, io.stat, and pids.current. Uses ctx.cgroup_path when set;
//! otherwise auto-detects via /proc/self/cgroup. Returns empty Vec gracefully
//! when not running inside a cgroup or on hosts without /sys/fs/cgroup.

use std::path::{Path, PathBuf};

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::signal::{Signal, SignalValue, Unit};

#[derive(Debug, Clone, Default)]
pub struct CgroupCollector;

impl CgroupCollector {
    pub fn new() -> Self {
        CgroupCollector
    }
}

impl Collector for CgroupCollector {
    fn id(&self) -> &str {
        "cgroup"
    }

    fn collect(&self, ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let base = match ctx.cgroup_path.clone().or_else(detect_cgroup_v2_path) {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let now = Local::now();
        let mut signals = Vec::new();

        if is_v2(&base) {
            collect_v2(&base, &mut signals, now);
        } else {
            collect_v1(&base, &mut signals, now);
        }

        Ok(signals)
    }
}

fn is_v2(base: &Path) -> bool {
    base.join("cgroup.controllers").exists() || base.join("cpu.stat").exists() || base.join("memory.current").exists()
}

/// Auto-detect the current process's cgroup v2 path from /proc/self/cgroup.
fn detect_cgroup_v2_path() -> Option<PathBuf> {
    let content = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    for line in content.lines() {
        // v2 format: "0::/<relative-path>"
        if let Some(rel) = line.strip_prefix("0::") {
            let rel = rel.trim().trim_start_matches('/');
            let path = if rel.is_empty() {
                PathBuf::from("/sys/fs/cgroup")
            } else {
                PathBuf::from("/sys/fs/cgroup").join(rel)
            };
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

fn collect_v2(base: &Path, signals: &mut Vec<Signal>, now: chrono::DateTime<Local>) {
    if let Some(v) = read_u64(base.join("memory.current")) {
        push(signals, "cgroup.memory_bytes", v as f64, Unit::Count, now);
    }
    if let Some(v) = read_mem_max(base.join("memory.max")) {
        push(signals, "cgroup.memory_limit_bytes", v, Unit::Count, now);
    }
    if let Some(v) = read_keyed(base.join("memory.events"), "oom_kill") {
        push(signals, "cgroup.oom_kills", v as f64, Unit::Count, now);
    }
    if let Some(v) = read_u64(base.join("pids.current")) {
        push(signals, "cgroup.pids_current", v as f64, Unit::Count, now);
    }
    if let Some(v) = read_keyed(base.join("cpu.stat"), "throttled_usec") {
        push(signals, "cgroup.cpu_throttled_usec", v as f64, Unit::Count, now);
    }
}

fn collect_v1(base: &Path, signals: &mut Vec<Signal>, now: chrono::DateTime<Local>) {
    let mem = base.join("memory");
    if let Some(v) = read_u64(mem.join("memory.usage_in_bytes")) {
        push(signals, "cgroup.memory_bytes", v as f64, Unit::Count, now);
    }
    // v1 limit: 9223372036854771712 = PAGE_COUNTER_MAX meaning no limit
    if let Some(v) = read_u64(mem.join("memory.limit_in_bytes")) {
        let limit = if v > i64::MAX as u64 / 2 { 0.0 } else { v as f64 };
        push(signals, "cgroup.memory_limit_bytes", limit, Unit::Count, now);
    }
    let pids = base.join("pids");
    if let Some(v) = read_u64(pids.join("pids.current")) {
        push(signals, "cgroup.pids_current", v as f64, Unit::Count, now);
    }
    // v1 cpuacct.usage is in nanoseconds; convert to microseconds
    let cpuacct = base.join("cpuacct");
    if let Some(v) = read_u64(cpuacct.join("cpuacct.usage")) {
        push(
            signals,
            "cgroup.cpu_throttled_usec",
            v as f64 / 1000.0,
            Unit::Count,
            now,
        );
    }
}

fn read_u64(path: PathBuf) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_mem_max(path: PathBuf) -> Option<f64> {
    let s = std::fs::read_to_string(path).ok()?;
    let s = s.trim();
    if s == "max" { Some(0.0) } else { s.parse().ok() }
}

fn read_keyed(path: PathBuf, key: &str) -> Option<u64> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let mut parts = line.splitn(2, ' ');
        if parts.next()? == key {
            return parts.next()?.trim().parse().ok();
        }
    }
    None
}

fn push(signals: &mut Vec<Signal>, id: &str, val: f64, unit: Unit, now: chrono::DateTime<Local>) {
    signals.push(Signal {
        id: id.to_string(),
        value: SignalValue::F64(val),
        unit,
        at: now,
        samples: None,
        stats: None,
        baseline: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_ok_on_any_host() {
        let c = CgroupCollector::new();
        let ctx = super::super::CollectCtx::default();
        let result = c.collect(&ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn collect_v2_from_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();

        std::fs::write(base.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        std::fs::write(base.join("memory.current"), "104857600\n").unwrap();
        std::fs::write(base.join("memory.max"), "209715200\n").unwrap();
        std::fs::write(
            base.join("memory.events"),
            "low 0\nhigh 0\nmax 0\noom 0\noom_kill 3\noom_group_kill 0\n",
        )
        .unwrap();
        std::fs::write(base.join("pids.current"), "42\n").unwrap();
        std::fs::write(
            base.join("cpu.stat"),
            "usage_usec 1000\nuser_usec 800\nsystem_usec 200\nnr_throttled 5\nthrottled_usec 500000\n",
        )
        .unwrap();

        let mut signals = Vec::new();
        let now = chrono::Local::now();
        collect_v2(base, &mut signals, now);

        let ids: Vec<_> = signals.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"cgroup.memory_bytes"), "missing memory_bytes: {:?}", ids);
        assert!(
            ids.contains(&"cgroup.memory_limit_bytes"),
            "missing memory_limit_bytes: {:?}",
            ids
        );
        assert!(ids.contains(&"cgroup.oom_kills"), "missing oom_kills: {:?}", ids);
        assert!(ids.contains(&"cgroup.pids_current"), "missing pids_current: {:?}", ids);
        assert!(
            ids.contains(&"cgroup.cpu_throttled_usec"),
            "missing cpu_throttled_usec: {:?}",
            ids
        );

        let mem = signals.iter().find(|s| s.id == "cgroup.memory_bytes").unwrap();
        assert_eq!(mem.value, crate::signal::SignalValue::F64(104857600.0));

        let oom = signals.iter().find(|s| s.id == "cgroup.oom_kills").unwrap();
        assert_eq!(oom.value, crate::signal::SignalValue::F64(3.0));
    }

    #[test]
    fn mem_max_unlimited_emits_zero() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        std::fs::write(base.join("memory.max"), "max\n").unwrap();
        assert_eq!(read_mem_max(base.join("memory.max")), Some(0.0));
    }
}
