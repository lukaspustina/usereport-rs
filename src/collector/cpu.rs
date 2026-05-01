//! CPU collector — `/proc/stat` delta engine.
//!
//! Two snapshots ≥ 1 s apart give the per-second rates (SDD §107). On hosts
//! without `/proc/stat` (e.g. macOS) the runtime collector falls back to
//! emitting an empty `Vec<Signal>`; the parser-based mpstat path is Phase 3
//! follow-up work.

use std::time::{Duration, Instant};

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::signal::{Signal, SignalValue, Unit};

/// Default minimum sampling window for the delta engine. The runtime
/// collector sleeps to reach this window if the elapsed time is shorter
/// (SDD §107).
const MIN_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct CpuCollector;

impl CpuCollector {
    pub fn new() -> Self {
        CpuCollector
    }

    /// Pure delta-engine entry point: parse two `/proc/stat` snapshots,
    /// compute per-second rate signals over `elapsed_secs`. Used in tests
    /// with synthetic snapshots; the runtime path in `collect()` reads
    /// `/proc/stat` twice with a sleep in between and calls this function.
    pub fn from_proc_stat_snapshots(s1: &str, s2: &str, elapsed_secs: f64) -> Vec<Signal> {
        let now = Local::now();
        let mut signals = Vec::new();
        let cpu1 = parse_cpu_aggregate(s1);
        let cpu2 = parse_cpu_aggregate(s2);
        if let (Some(a), Some(b)) = (cpu1, cpu2) {
            let total_delta = b.total().saturating_sub(a.total()) as f64;
            if total_delta > 0.0 {
                let usr = (b.user.saturating_sub(a.user) as f64 / total_delta) * 100.0;
                let sys = (b.system.saturating_sub(a.system) as f64 / total_delta) * 100.0;
                let iow = (b.iowait.saturating_sub(a.iowait) as f64 / total_delta) * 100.0;
                let idle = (b.idle.saturating_sub(a.idle) as f64 / total_delta) * 100.0;
                push(&mut signals, "cpu.usr_pct", usr, Unit::Pct, now);
                push(&mut signals, "cpu.sys_pct", sys, Unit::Pct, now);
                push(&mut signals, "cpu.iowait_pct", iow, Unit::Pct, now);
                push(&mut signals, "cpu.idle_pct", idle, Unit::Pct, now);
            }
        }
        if let Some(r) = parse_procs_running(s2) {
            push(&mut signals, "vmstat.r", r as f64, Unit::Count, now);
        }
        // Context switch rate (per second) — useful for future rules; harmless
        // if elapsed_secs <= 0.
        if elapsed_secs > 0.0 {
            if let (Some(a), Some(b)) = (parse_ctxt(s1), parse_ctxt(s2)) {
                let delta = b.saturating_sub(a) as f64;
                push(&mut signals, "cpu.ctxt_per_sec", delta / elapsed_secs, Unit::Count, now);
            }
        }
        signals
    }
}

impl Collector for CpuCollector {
    fn id(&self) -> &str {
        "cpu"
    }

    fn supports_sampling(&self) -> bool {
        true
    }

    /// Read `/proc/stat` with a 1 s minimum window. When `ctx.duration` and
    /// `ctx.interval` are set, loop N = floor(duration/interval)+1 times,
    /// collecting one snapshot per interval, and populate `Signal::samples`.
    /// On hosts without `/proc/stat`, return Ok(empty).
    fn collect(&self, ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let s1 = match std::fs::read_to_string("/proc/stat") {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };

        if let (Some(duration), Some(interval)) = (ctx.duration, ctx.interval) {
            return self.collect_sampled(&s1, duration, interval);
        }

        // Single-shot: one delta over MIN_WINDOW.
        let started = Instant::now();
        std::thread::sleep(MIN_WINDOW);
        let elapsed_secs = started.elapsed().as_secs_f64().max(1.0);
        let s2 = match std::fs::read_to_string("/proc/stat") {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };
        Ok(Self::from_proc_stat_snapshots(&s1, &s2, elapsed_secs))
    }
}

impl CpuCollector {
    fn collect_sampled(&self, first_snapshot: &str, duration: Duration, interval: Duration) -> Result<Vec<Signal>> {
        let n = (duration.as_secs_f64() / interval.as_secs_f64()).floor() as usize + 1;
        // Maps signal id → (unit, vec of per-sample f64 values).
        let mut samples: std::collections::HashMap<String, (crate::signal::Unit, Vec<f64>)> =
            std::collections::HashMap::new();
        let mut prev = first_snapshot.to_string();
        for _ in 0..n {
            let sleep_for = interval.max(MIN_WINDOW);
            let started = Instant::now();
            std::thread::sleep(sleep_for);
            let elapsed_secs = started.elapsed().as_secs_f64().max(1.0);
            let next = match std::fs::read_to_string("/proc/stat") {
                Ok(s) => s,
                Err(_) => break,
            };
            for sig in Self::from_proc_stat_snapshots(&prev, &next, elapsed_secs) {
                if let Some(v) = sig.value.as_f64() {
                    samples.entry(sig.id.clone()).or_insert_with(|| (sig.unit, Vec::new())).1.push(v);
                }
            }
            prev = next;
        }
        if samples.is_empty() {
            return Ok(Vec::new());
        }
        let now = chrono::Local::now();
        let mut signals: Vec<Signal> = samples
            .into_iter()
            .map(|(id, (unit, vals))| {
                let value = vals.last().copied().unwrap_or(0.0);
                Signal {
                    id,
                    value: crate::signal::SignalValue::F64(value),
                    unit,
                    at: now,
                    samples: Some(vals),
                    baseline: None,
                }
            })
            .collect();
        signals.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(signals)
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    guest: u64,
    guest_n: u64,
}

impl CpuTimes {
    fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
            + self.guest
            + self.guest_n
    }
}

fn parse_cpu_aggregate(s: &str) -> Option<CpuTimes> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("cpu ").or_else(|| line.strip_prefix("cpu  ")) {
            let nums: Vec<u64> = rest.split_whitespace().filter_map(|t| t.parse::<u64>().ok()).collect();
            if nums.len() < 4 {
                return None;
            }
            return Some(CpuTimes {
                user: *nums.first().unwrap_or(&0),
                nice: *nums.get(1).unwrap_or(&0),
                system: *nums.get(2).unwrap_or(&0),
                idle: *nums.get(3).unwrap_or(&0),
                iowait: *nums.get(4).unwrap_or(&0),
                irq: *nums.get(5).unwrap_or(&0),
                softirq: *nums.get(6).unwrap_or(&0),
                steal: *nums.get(7).unwrap_or(&0),
                guest: *nums.get(8).unwrap_or(&0),
                guest_n: *nums.get(9).unwrap_or(&0),
            });
        }
    }
    None
}

fn parse_procs_running(s: &str) -> Option<u64> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("procs_running ") {
            return rest.trim().parse::<u64>().ok();
        }
    }
    None
}

fn parse_ctxt(s: &str) -> Option<u64> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("ctxt ") {
            return rest.trim().parse::<u64>().ok();
        }
    }
    None
}

fn push(signals: &mut Vec<Signal>, id: &str, v: f64, unit: Unit, at: chrono::DateTime<Local>) {
    signals.push(Signal {
        id: id.to_string(),
        value: SignalValue::F64(v),
        unit,
        at,
        samples: None,
        baseline: None,
    });
}
