//! Host-level collector: emits cpu_count, mem_total_bytes, load_avg_1m.

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
        let cpu_count;
        let mem_total;
        let load_avg;

        match super::platform::read_host_snapshot() {
            Some(snap) => {
                cpu_count = snap.cpu_count as f64;
                mem_total = snap.mem_total_bytes as f64;
                load_avg = snap.load_avg_1m;
            }
            None => {
                cpu_count =
                    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
                mem_total = 0.0;
                load_avg = 0.0;
            }
        }

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
