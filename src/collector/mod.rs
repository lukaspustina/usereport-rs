//! `Collector` trait and shared collection context.
//!
//! Each collector emits a `Vec<Signal>`. Implementations may either parse a
//! tool's stdout (e.g. `vmstat`, `free`, `iostat`) or read `/proc`/`/sys`
//! directly (Phase 3).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use crate::signal::Signal;

#[cfg(feature = "bpf")]
pub mod bpf;
pub mod cgroup;
pub mod cpu;
pub mod cpufreq;
pub mod disk;
pub mod dmesg;
pub mod host;
pub mod interrupts;
pub mod memory;
pub mod network;
pub mod platform;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read {path}: {source}")]
    ReadFailed { path: PathBuf, source: std::io::Error },
    #[error("failed to parse output of '{collector}': {reason}")]
    ParseFailed { collector: String, reason: String },
    #[error("collector '{collector}' is unavailable on this host: {reason}")]
    Unavailable { collector: String, reason: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Context passed to every `Collector::collect` call.
#[derive(Debug, Default, Clone)]
pub struct CollectCtx {
    pub duration: Option<Duration>,
    pub interval: Option<Duration>,
    pub cgroup_path: Option<PathBuf>,
    pub baseline: Option<Arc<()>>,
    pub cpu_count: usize,
}

pub trait Collector: std::fmt::Debug + Send + Sync {
    fn id(&self) -> &str;
    fn collect(&self, ctx: &CollectCtx) -> Result<Vec<Signal>>;
    fn supports_sampling(&self) -> bool {
        false
    }
}
