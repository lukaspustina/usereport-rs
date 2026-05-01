//! Disk collector — Phase 1 stub.
//!
//! Full iostat parsing and direct `/proc/diskstats` delta engine are deferred
//! to Phase 3. Stub satisfies SDD §157.

use super::{CollectCtx, Collector, Result};
use crate::signal::Signal;

#[derive(Debug, Clone, Default)]
pub struct DiskCollector;

impl Collector for DiskCollector {
    fn id(&self) -> &str {
        "disk"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        // TODO(Phase 3): parse iostat output and emit disk.* signals.
        Ok(vec![])
    }
}
