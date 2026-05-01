//! Host-level collector: emits cpu_count, mem_total_bytes, load_avg_1m.
//!
//! Reads from `/proc/loadavg` and `/proc/meminfo` on Linux; falls back
//! gracefully on macOS and other systems (returns zero values).

use chrono::Local;

use crate::collector::{CollectCtx, Result};
use crate::signal::{Signal, SignalValue, Unit};

#[derive(Debug, Default)]
pub struct HostCollector;

impl HostCollector {
    pub fn new() -> Self {
        Self
    }
}

impl super::Collector for HostCollector {
    fn id(&self) -> &str {
        "host"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let now = Local::now();
        let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
        let mem_total = read_mem_total_bytes().unwrap_or(0) as f64;
        let load_avg = read_load_avg_1m().unwrap_or(0.0);

        Ok(vec![
            Signal {
                id: "host.cpu_count".to_string(),
                value: SignalValue::F64(cpu_count),
                unit: Unit::None,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            },
            Signal {
                id: "host.mem_total_bytes".to_string(),
                value: SignalValue::F64(mem_total),
                unit: Unit::None,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            },
            Signal {
                id: "host.load_avg_1m".to_string(),
                value: SignalValue::F64(load_avg),
                unit: Unit::None,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            },
        ])
    }
}

fn read_mem_total_bytes() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

fn read_load_avg_1m() -> Option<f64> {
    let content = std::fs::read_to_string("/proc/loadavg").ok()?;
    content.split_whitespace().next()?.parse().ok()
}
