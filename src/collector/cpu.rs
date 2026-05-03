//! CPU collector — `/proc/stat` delta engine (Linux) and `kern.cp_time` (macOS).

use std::time::{Duration, Instant};

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::baseline::stats::sample_stats;
use crate::collector::platform::{CpuSnapshot, read_cpu_snapshot};
use crate::signal::{Signal, SignalValue, Unit};

const MIN_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct CpuCollector;

impl CpuCollector {
    pub fn new() -> Self {
        CpuCollector
    }

    /// Pure delta-engine entry point: parse two `/proc/stat` snapshots,
    /// compute per-second rate signals over `elapsed_secs`. Used in tests
    /// with synthetic snapshots.
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
            push(&mut signals, "cpu.run_queue", r as f64, Unit::Count, now);
        }
        if elapsed_secs > 0.0 {
            if let (Some(a), Some(b)) = (parse_ctxt(s1), parse_ctxt(s2)) {
                let delta = b.saturating_sub(a) as f64;
                push(&mut signals, "cpu.ctxt_per_sec", delta / elapsed_secs, Unit::Count, now);
            }
        }
        signals
    }

    /// Snapshot-based delta engine: compute signals from two `CpuSnapshot`s.
    /// Emits `cpu.iowait_pct` only when both snapshots have `iowait: Some`.
    /// Emits `cpu.run_queue` only when `b.procs_running` is `Some`.
    /// Emits `cpu.ctxt_per_sec` only when both snapshots have `ctxt: Some`.
    pub fn from_cpu_snapshots(a: &CpuSnapshot, b: &CpuSnapshot, elapsed_secs: f64) -> Vec<Signal> {
        let now = Local::now();
        let mut signals = Vec::new();

        let total_delta = b.total().saturating_sub(a.total()) as f64;
        if total_delta > 0.0 {
            let pct =
                |b_field: u64, a_field: u64| -> f64 { (b_field.saturating_sub(a_field) as f64 / total_delta) * 100.0 };
            push(&mut signals, "cpu.usr_pct", pct(b.user, a.user), Unit::Pct, now);
            push(&mut signals, "cpu.sys_pct", pct(b.system, a.system), Unit::Pct, now);
            push(&mut signals, "cpu.idle_pct", pct(b.idle, a.idle), Unit::Pct, now);

            if let (Some(ai), Some(bi)) = (a.iowait, b.iowait) {
                let iow = (bi.saturating_sub(ai) as f64 / total_delta) * 100.0;
                push(&mut signals, "cpu.iowait_pct", iow, Unit::Pct, now);
            }
        }

        if let Some(r) = b.procs_running {
            push(&mut signals, "cpu.run_queue", r as f64, Unit::Count, now);
        }

        if elapsed_secs > 0.0 {
            if let (Some(ac), Some(bc)) = (a.ctxt, b.ctxt) {
                let delta = bc.saturating_sub(ac) as f64;
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

    #[cfg(target_os = "linux")]
    fn source_commands(&self) -> &[&str] {
        &["sar_cpu", "mpstat", "iostat"]
    }

    #[cfg(target_os = "macos")]
    fn source_commands(&self) -> &[&str] {
        &["vm_stat", "iostat"]
    }

    fn collect(&self, ctx: &CollectCtx) -> Result<Vec<Signal>> {
        if let (Some(duration), Some(interval)) = (ctx.duration, ctx.interval) {
            return self.collect_sampled(duration, interval);
        }

        let started = Instant::now();
        let s1 = match read_cpu_snapshot() {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        std::thread::sleep(MIN_WINDOW);
        let elapsed_secs = started.elapsed().as_secs_f64().max(1.0);
        let s2 = match read_cpu_snapshot() {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        Ok(Self::from_cpu_snapshots(&s1, &s2, elapsed_secs))
    }
}

impl CpuCollector {
    fn collect_sampled(&self, duration: Duration, interval: Duration) -> Result<Vec<Signal>> {
        let n = (duration.as_secs_f64() / interval.as_secs_f64()).floor() as usize + 1;
        let mut samples: std::collections::HashMap<String, (crate::signal::Unit, Vec<f64>)> =
            std::collections::HashMap::new();
        let mut prev = match read_cpu_snapshot() {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        for _ in 0..n {
            let sleep_for = interval.max(MIN_WINDOW);
            let started = Instant::now();
            std::thread::sleep(sleep_for);
            let elapsed_secs = started.elapsed().as_secs_f64().max(1.0);
            let next = match read_cpu_snapshot() {
                Some(s) => s,
                None => break,
            };
            for sig in Self::from_cpu_snapshots(&prev, &next, elapsed_secs) {
                if let Some(v) = sig.value.as_f64() {
                    samples
                        .entry(sig.id.clone())
                        .or_insert_with(|| (sig.unit, Vec::new()))
                        .1
                        .push(v);
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
                let stats = sample_stats(&vals);
                Signal {
                    id,
                    value: crate::signal::SignalValue::F64(value),
                    unit,
                    at: now,
                    samples: Some(vals),
                    stats,
                    baseline: None,
                }
            })
            .collect();
        signals.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(signals)
    }
}

// ---------------------------------------------------------------------------
// Legacy /proc/stat helpers — preserved for from_proc_stat_snapshots()
// Phase 7 will remove these when from_proc_stat_snapshots is retired.
// ---------------------------------------------------------------------------

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
        stats: None,
        baseline: None,
    });
}
