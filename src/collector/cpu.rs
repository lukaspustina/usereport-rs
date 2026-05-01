//! CPU collector — Phase 1 stub.
//!
//! Full vmstat/mpstat parsing and direct `/proc/stat` delta engine are
//! deferred to Phase 3. This module exists to satisfy SDD §157 ("File &
//! Module Structure") and to anchor the public path for downstream code.

use super::{CollectCtx, Collector, Result};
use crate::signal::Signal;

#[derive(Debug, Clone, Default)]
pub struct CpuCollector;

impl Collector for CpuCollector {
    fn id(&self) -> &str {
        "cpu"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        // TODO(Phase 3): parse vmstat/mpstat output and emit cpu.* signals.
        Ok(vec![])
    }
}
