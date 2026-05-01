//! CPU frequency and thermal collector.
//!
//! Emits `cpu.freq_ratio` and `cpu.temp_celsius` where available.
//! Returns empty Vec on macOS (no portable cross-chip sysctl).

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
        let snap = super::platform::read_cpufreq_snapshot();
        let mut signals = Vec::new();

        if let Some(ratio) = snap.freq_ratio {
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
        if let Some(temp) = snap.temp_celsius {
            signals.push(Signal {
                id: "cpu.temp_celsius".to_string(),
                value: SignalValue::F64(temp),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_ok_on_any_host() {
        let c = CpuFreqCollector::new();
        let ctx = super::super::CollectCtx::default();
        let result = c.collect(&ctx);
        assert!(result.is_ok(), "collect failed: {:?}", result);
    }
}
