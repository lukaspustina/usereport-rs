//! Memory collector — reads via platform::read_mem_snapshot() on both platforms.

use chrono::Local;

use super::{CollectCtx, Collector, Error, Result};
use crate::collector::platform::{MemSnapshot, read_mem_snapshot};
use crate::signal::{Signal, SignalValue, Unit};

#[derive(Debug, Clone, Default)]
pub struct MemoryCollector {
    /// Stored `free -m` output for the `from_stdout` legacy test path.
    stdout: Option<String>,
}

impl MemoryCollector {
    /// Runtime constructor: no pre-captured output; `collect()` calls
    /// `platform::read_mem_snapshot()` at runtime.
    pub fn new() -> Self {
        MemoryCollector { stdout: None }
    }

    /// Build a collector from a captured `free -m` stdout buffer (test path).
    pub fn from_stdout(stdout: String) -> Self {
        MemoryCollector { stdout: Some(stdout) }
    }

    /// Convert a `MemSnapshot` to signals. Used by `collect()` on both platforms.
    pub fn signals_from_mem_snapshot(snap: &MemSnapshot) -> Result<Vec<Signal>> {
        let now = Local::now();
        let mut signals = Vec::new();

        push(&mut signals, "mem.total_mb", snap.total_mb, Unit::Count, now);
        push(&mut signals, "mem.used_mb", snap.used_mb, Unit::Count, now);
        push(&mut signals, "mem.free_mb", snap.free_mb, Unit::Count, now);

        if let Some(avail) = snap.available_mb {
            push(&mut signals, "mem.available_mb", avail, Unit::Count, now);
        }

        if snap.total_mb > 0.0 {
            push(&mut signals, "mem.free_pct", snap.free_mb / snap.total_mb * 100.0, Unit::Pct, now);
        }

        push(&mut signals, "swap.total_mb", snap.swap_total_mb, Unit::Count, now);
        push(&mut signals, "swap.used_mb", snap.swap_used_mb, Unit::Count, now);
        push(&mut signals, "swap.free_mb", snap.swap_free_mb, Unit::Count, now);

        Ok(signals)
    }
}

impl Collector for MemoryCollector {
    fn id(&self) -> &str {
        "memory"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        // Legacy test path: use pre-captured stdout.
        if let Some(ref s) = self.stdout {
            return parse_free_output(s);
        }

        // Runtime path: call platform function (same on Linux and macOS).
        match read_mem_snapshot() {
            Some(snap) => Self::signals_from_mem_snapshot(&snap),
            None => Ok(Vec::new()),
        }
    }
}

fn parse_free_output(s: &str) -> Result<Vec<Signal>> {
    let now = Local::now();
    let mut signals = Vec::new();
    let mut mem_total: Option<f64> = None;
    let mut mem_free: Option<f64> = None;

    for line in s.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Mem:") {
            let nums = numeric_tokens(rest);
            if nums.len() < 3 {
                return Err(Error::ParseFailed {
                    collector: "memory".to_string(),
                    reason: format!("Mem: needs at least 3 numbers, got {}", nums.len()),
                });
            }
            let total = nums[0];
            let used = nums[1];
            let free = nums[2];
            mem_total = Some(total);
            mem_free = Some(free);
            push(&mut signals, "mem.total_mb", total, Unit::Count, now);
            push(&mut signals, "mem.used_mb", used, Unit::Count, now);
            push(&mut signals, "mem.free_mb", free, Unit::Count, now);
            if let Some(available) = nums.get(5) {
                push(&mut signals, "mem.available_mb", *available, Unit::Count, now);
            }
        } else if let Some(rest) = trimmed.strip_prefix("Swap:") {
            let nums = numeric_tokens(rest);
            if nums.len() < 3 {
                return Err(Error::ParseFailed {
                    collector: "memory".to_string(),
                    reason: format!("Swap: needs at least 3 numbers, got {}", nums.len()),
                });
            }
            push(&mut signals, "swap.total_mb", nums[0], Unit::Count, now);
            push(&mut signals, "swap.used_mb", nums[1], Unit::Count, now);
            push(&mut signals, "swap.free_mb", nums[2], Unit::Count, now);
        }
    }

    if let (Some(total), Some(free)) = (mem_total, mem_free) {
        if total > 0.0 {
            push(&mut signals, "mem.free_pct", free / total * 100.0, Unit::Pct, now);
        }
    }

    Ok(signals)
}

fn numeric_tokens(s: &str) -> Vec<f64> {
    s.split_whitespace().filter_map(|t| t.parse::<f64>().ok()).collect()
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
