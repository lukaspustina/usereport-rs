//! CPU frequency and thermal collector (SDD Req 11).
//!
//! Reads `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` and
//! `/sys/devices/system/cpu/cpu*/cpufreq/scaling_max_freq` to emit
//! `cpu.freq_ratio`. Reads `/sys/class/thermal/thermal_zone*/temp` for
//! `cpu.temp_celsius`. Returns empty Vec gracefully on macOS / kernels without
//! cpufreq support.

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::signal::{Signal, SignalValue, Unit};

#[derive(Debug, Clone, Default)]
pub struct CpuFreqCollector;

impl CpuFreqCollector {
    pub fn new() -> Self {
        CpuFreqCollector
    }
}

impl Collector for CpuFreqCollector {
    fn id(&self) -> &str {
        "cpufreq"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let now = Local::now();
        let mut signals = Vec::new();

        // Frequency ratio: mean(cur) / mean(max) across all CPUs.
        let (cur_sum, max_sum, count) = read_freq_khz();
        if count > 0 {
            let ratio = (cur_sum / count as f64) / (max_sum / count as f64);
            signals.push(Signal {
                id: "cpu.freq_ratio".to_string(),
                value: SignalValue::F64(ratio),
                unit: Unit::None,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            });
        }

        // Thermal: max temperature across all thermal zones (millidegrees → Celsius).
        if let Some(max_temp) = read_max_temp_celsius() {
            signals.push(Signal {
                id: "cpu.temp_celsius".to_string(),
                value: SignalValue::F64(max_temp),
                unit: Unit::Celsius,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            });
        }

        Ok(signals)
    }
}

/// Returns (sum_cur_khz, sum_max_khz, count) across all CPUs that expose cpufreq.
fn read_freq_khz() -> (f64, f64, usize) {
    let mut cur_sum = 0.0f64;
    let mut max_sum = 0.0f64;
    let mut count = 0usize;

    let base = std::path::Path::new("/sys/devices/system/cpu");
    let Ok(entries) = std::fs::read_dir(base) else { return (0.0, 0.0, 0) };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Only cpu0, cpu1, … (not cpufreq, cpuidle, etc.)
        if !name.starts_with("cpu") || !name[3..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let cur_path = entry.path().join("cpufreq/scaling_cur_freq");
        let max_path = entry.path().join("cpufreq/scaling_max_freq");
        if let (Ok(cur_s), Ok(max_s)) = (std::fs::read_to_string(&cur_path), std::fs::read_to_string(&max_path)) {
            let cur: f64 = cur_s.trim().parse().unwrap_or(0.0);
            let max: f64 = max_s.trim().parse().unwrap_or(0.0);
            if max > 0.0 {
                cur_sum += cur;
                max_sum += max;
                count += 1;
            }
        }
    }
    (cur_sum, max_sum, count)
}

/// Returns the maximum temperature in Celsius across all thermal zones.
fn read_max_temp_celsius() -> Option<f64> {
    let base = std::path::Path::new("/sys/class/thermal");
    let entries = std::fs::read_dir(base).ok()?;
    let mut max_mc: i64 = i64::MIN;
    let mut found = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        let temp_path = entry.path().join("temp");
        if let Ok(s) = std::fs::read_to_string(temp_path) {
            if let Ok(mc) = s.trim().parse::<i64>() {
                if mc > max_mc {
                    max_mc = mc;
                    found = true;
                }
            }
        }
    }
    if found { Some(max_mc as f64 / 1000.0) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_ok_on_any_host() {
        // Should not panic or error even on macOS without /sys.
        let c = CpuFreqCollector::new();
        let ctx = super::super::CollectCtx::default();
        let result = c.collect(&ctx);
        assert!(result.is_ok(), "collect failed: {:?}", result);
    }
}
