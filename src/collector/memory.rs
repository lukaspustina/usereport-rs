//! Parser-based memory collector reading `free -m` output.
//!
//! Direct `/proc/meminfo` reading is Phase 3.

use chrono::Local;

use super::{CollectCtx, Collector, Error, Result};
use crate::signal::{Signal, SignalValue, Unit};

#[derive(Debug, Clone)]
pub struct MemoryCollector {
    stdout: String,
}

impl MemoryCollector {
    /// Build a collector from a captured `free -m` stdout buffer (used in
    /// tests and as a stable interface for the runner once it pipes captured
    /// command output into the diagnostic pipeline).
    pub fn from_stdout(stdout: String) -> Self {
        MemoryCollector { stdout }
    }
}

impl Collector for MemoryCollector {
    fn id(&self) -> &str {
        "memory"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        parse_free_output(&self.stdout)
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
            // free(1) -m columns: total used free shared buff/cache available
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
            let pct = free / total * 100.0;
            push(&mut signals, "mem.free_pct", pct, Unit::Pct, now);
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
