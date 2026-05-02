//! Platform-specific snapshot types and the sole location for all
//! `#[cfg(target_os)]` attributes in the `src/collector/` subtree.
//!
//! Six functions with identical signatures on Linux and macOS are re-exported
//! here; callers import them via `super::platform::read_*`.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Snapshot types (platform-neutral data model)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    /// Always `None` on macOS; `Some` on Linux.
    pub iowait: Option<u64>,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
    /// Always `None` on macOS (no instantaneous runnable-thread count).
    pub procs_running: Option<u64>,
    pub ctxt: Option<u64>,
}

impl CpuSnapshot {
    /// Total tick count used for computing percentages.
    pub fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait.unwrap_or(0)
            + self.irq
            + self.softirq
            + self.steal
    }
}

#[derive(Debug, Clone)]
pub struct NetSnapshot {
    pub rx_drops: HashMap<String, u64>,
    pub tcp_out_segs: u64,
    pub tcp_retrans_segs: u64,
    /// Cumulative failed TCP connection attempts (`Tcp: AttemptFails` on Linux;
    /// `bad connection attempt` count on macOS). Used as a delta between two
    /// snapshots to emit `net.connect_failures`.
    pub tcp_attempt_fails: u64,
    /// Cumulative resets sent from ESTABLISHED or CLOSE_WAIT (`Tcp: EstabResets`
    /// on Linux). Used as a delta to emit `net.estab_resets`. Always 0 on macOS
    /// (no equivalent counter in `netstat -s -p tcp`).
    pub tcp_estab_resets: u64,
    /// `None` when unavailable (macOS: parse failure only; Linux: sockstat missing).
    pub tcp_tw_count: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct HostSnapshot {
    pub cpu_count: u64,
    pub mem_total_bytes: u64,
    pub load_avg_1m: f64,
}

#[derive(Debug, Clone)]
pub struct MemSnapshot {
    pub total_mb: f64,
    pub used_mb: f64,
    pub free_mb: f64,
    /// Always `None` on macOS.
    pub available_mb: Option<f64>,
    pub swap_total_mb: f64,
    pub swap_used_mb: f64,
    pub swap_free_mb: f64,
    /// Cumulative pages swapped in since boot (`pswpin` on Linux; `Swapins` on
    /// macOS). Used as a delta between two snapshots to emit `vmstat.swap_in`.
    pub swap_in_pages: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CpuFreqSnapshot {
    /// `None` on macOS (no portable cross-chip sysctl).
    pub freq_ratio: Option<f64>,
    /// `None` on macOS.
    pub temp_celsius: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct DiskDevSnapshot {
    pub name: String,
    pub read_ios: u64,
    pub write_ios: u64,
    /// `None` on macOS (not exposed without root).
    pub read_time_ms: Option<u64>,
    /// `None` on macOS.
    pub write_time_ms: Option<u64>,
    /// `None` on macOS.
    pub io_time_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Platform function re-exports — ALL #[cfg(target_os)] live here and nowhere
// else in src/collector/.
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
