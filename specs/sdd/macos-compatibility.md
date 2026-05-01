# SDD: macOS Collector Parity

**Status:** Ready for Implementation
**Original:** specs/sdd/macos-compatibility.md
**Refined:** 2026-05-01

---

## Overview

All direct-kernel collectors (`cpu`, `network`, `disk`, `host`, `memory`, `cpufreq`) return empty signal lists on macOS, so the rule engine fires no findings there. This SDD introduces `src/collector/platform/` as the sole location for `#[cfg(target_os)]` attributes, adds native macOS signal collection via `sysctl`, `netstat`, and `vm_stat`, and registers `MemoryCollector` in the CLI collector list (currently absent on all platforms).

---

## Context & Constraints

- Rust 2024 edition, MSRV 1.85. `cargo test --all-features` must stay green on Linux and macOS.
- No new crate dependencies. Use `std::process::Command` for OS subprocess calls. Existing deps available: `subprocess`, `which`, `thiserror`, `rustix`.
- No root-requiring tools: no `dtrace`, no `powermetrics`, no `perf`.
- Every `#[cfg(target_os = "linux")]` and `#[cfg(target_os = "macos")]` attribute in `src/collector/` must live exclusively in `src/collector/platform/mod.rs`. Zero cfg-target-os in any other file under `src/collector/`.
- Prefer editing existing files. No speculative abstractions. Surgical changes only.
- When a sysctl or Command call fails (Err or empty stdout), return `None` / empty `Vec` — never panic.
- Parse functions must handle unexpected output by returning `None` / empty rather than panicking.

---

## Architecture

```
src/collector/
  platform/
    mod.rs      ← ALL #[cfg(target_os)] in the collector tree (only here)
                   defines snapshot types; re-exports six platform functions
    linux.rs    ← /proc + /sys readers extracted from existing collectors
    macos.rs    ← sysctl + netstat + vm_stat readers (new)
  cpu.rs        ← add from_cpu_snapshots(); update collect(); keep from_proc_stat_snapshots()
  network.rs    ← add from_net_snapshots(); update collect(); keep from_snapshots()
  disk.rs       ← add from_disk_snapshots(); update collect(); keep from_proc_diskstats_snapshots()
  host.rs       ← replace /proc helpers with platform::read_host_snapshot()
  cpufreq.rs    ← replace /sys helpers with platform::read_cpufreq_snapshot()
  memory.rs     ← add parse_vm_stat_output(), new(), update collect()
  mod.rs        ← add `pub mod platform;`
src/cli/mod.rs  ← add MemoryCollector::new() to collector list
```

---

## Requirements

**Platform module**

R1. The system shall introduce `src/collector/platform/mod.rs` as the sole file containing `#[cfg(target_os = "linux")]` and `#[cfg(target_os = "macos")]` attributes across the entire `src/collector/` subtree. This constraint applies to `src/collector/` only; existing `cfg(target_os)` usage in `src/cli/`, `src/command.rs`, `src/runner.rs`, and `src/analysis.rs` is out of scope.

R2. `platform/mod.rs` shall define all six platform-neutral snapshot structs and re-export the six platform functions via `#[cfg(target_os = "linux")] pub use linux::*;` and `#[cfg(target_os = "macos")] pub use macos::*;`.

R3. Both `platform/linux.rs` and `platform/macos.rs` shall implement the same six `pub fn` signatures with no `#[cfg]` attributes inside them:
   - `pub fn read_cpu_snapshot() -> Option<CpuSnapshot>`
   - `pub fn read_net_snapshot() -> Option<NetSnapshot>`
   - `pub fn read_host_snapshot() -> Option<HostSnapshot>`
   - `pub fn read_mem_snapshot() -> Option<MemSnapshot>`
   - `pub fn read_cpufreq_snapshot() -> CpuFreqSnapshot`
   - `pub fn read_disk_snapshots() -> Vec<DiskDevSnapshot>`

**Snapshot types**

R4. `CpuSnapshot` shall carry `iowait: Option<u64>` (always `None` on macOS because the macOS kernel does not expose an iowait counter).

R5. `DiskDevSnapshot` shall carry `read_time_ms: Option<u64>`, `write_time_ms: Option<u64>`, `io_time_ms: Option<u64>` (all `None` on macOS because `iostat` does not expose await or utilisation without root).

R6. `MemSnapshot` shall carry `available_mb: Option<f64>` (always `None` on macOS).

**Linux platform (`platform/linux.rs`)**

R7. `platform/linux.rs` shall implement all six functions by extracting the existing `/proc`/`/sys` parse logic from the individual collector files. No behavioural change on Linux is permitted.

- `read_cpu_snapshot()`: parse `/proc/stat` aggregate `cpu` line and `procs_running`/`ctxt` fields.
- `read_net_snapshot()`: parse `/proc/net/dev` for `rx_drops` per interface, `/proc/net/snmp` for `tcp_out_segs`/`tcp_retrans_segs`, `/proc/net/sockstat` for `tcp_tw_count`.
- `read_host_snapshot()`: parse `/proc/loadavg` first token, `/proc/meminfo` MemTotal, use `std::thread::available_parallelism()` for cpu count.
- `read_mem_snapshot()`: parse `free -m` stdout (run via `std::process::Command`); `available_mb` = column index 5 of the `Mem:` line.
- `read_cpufreq_snapshot()`: read `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` and `scaling_max_freq`; read `/sys/class/thermal/thermal_zone*/temp`.
- `read_disk_snapshots()`: parse `/proc/diskstats`.

**macOS platform (`platform/macos.rs`)**

R8. `read_host_snapshot()` shall run:
   - `sysctl -n hw.logicalcpu` → parse single integer → `cpu_count: u64`.
   - `sysctl -n hw.memsize` → parse single integer bytes → `mem_total_bytes: u64`.
   - `sysctl -n vm.loadavg` → parse first float from output of the form `{ 0.52 0.58 0.57 }` → `load_avg_1m: f64`.
   Return `None` if any Command fails or produces unparseable output.

R9. `read_cpu_snapshot()` shall:
   - Run `sysctl -n kern.cp_time` → parse five whitespace-separated integers: `user nice sys idle irq` → populate `CpuSnapshot { user, nice, system: sys, idle, irq, iowait: None, softirq: 0, steal: 0, procs_running: None, ctxt: None }`.
   - Run `sysctl -n vm.stats.sys.v_swtch` → parse single integer → `ctxt: Some(value)`.
   - Set `procs_running: None`. macOS does not expose an instantaneous runnable-thread count equivalent to Linux `/proc/stat`; using load average as a proxy (`vm.loadavg` first float cast to `u64`) produces values that diverge from the Linux metric semantics and silently misfires the `vmstat.r` rule.
   Return `None` if `kern.cp_time` fails or cannot be parsed.

R10. `read_net_snapshot()` shall:
   - Run `netstat -i -b -n`. For each data row (skip header lines that start with `Name`), skip the loopback interface (name == `lo0`). Parse the last whitespace-delimited field on each data row as the `Idrop` (input drop) counter: `rx_drops: HashMap<String, u64>` keyed by interface name. If a field is not a valid `u64`, skip that row.
   - Run `netstat -s -p tcp`. Parse `tcp_out_segs` from the line matching `\d+ packets sent` (first integer on that line). Parse `tcp_retrans_segs` from the line matching `\d+ data packets.*retransmitted` (first integer). Parse `tcp_tw_count` from the line matching `\d+ connections in TIME_WAIT` (first integer).
   Return `Some(NetSnapshot { rx_drops, tcp_out_segs, tcp_retrans_segs, tcp_tw_count })` with zero/empty defaults for fields that cannot be parsed (never return `None` unless both commands fail entirely).

R11. `read_mem_snapshot()` shall:
   - Run `vm_stat`. Extract page size from the header line: `Mach Virtual Memory Statistics: (page size of N bytes)` → `page_size: u64`.
   - Parse the following key-value lines (format `Key: value.`): `Pages free`, `Pages active`, `Pages inactive`, `Pages speculative`, `Pages wired down`, `Pages purgeable`.
   - Compute: `total_pages = active + inactive + speculative + wired + purgeable + free`. `total_mb = total_pages * page_size / 1_048_576`. `free_mb = (free + speculative) * page_size / 1_048_576`. `used_mb = total_mb - free_mb`.
   - Run `sysctl -n vm.swapusage`. Parse output of the form `total = X.XXM  used = Y.YYM  free = Z.ZZM`. Strip the trailing `M`/`G` suffix and convert to MB. `swap_total_mb`, `swap_used_mb`, `swap_free_mb`.
   - Set `available_mb: None`.
   Return `None` if `vm_stat` fails or page size cannot be parsed.

R12. `read_cpufreq_snapshot()` shall return `CpuFreqSnapshot { freq_ratio: None, temp_celsius: None }` on macOS unconditionally.

R13. `read_disk_snapshots()` shall return `Vec::new()` on macOS unconditionally.

**Collector refactors**

R14. `CpuCollector::collect()` shall call `platform::read_cpu_snapshot()` twice (with the existing `MIN_WINDOW` sleep between calls, recording elapsed time with `Instant`), then call a new `from_cpu_snapshots(a: &CpuSnapshot, b: &CpuSnapshot, elapsed_secs: f64) -> Vec<Signal>` function. The existing public `from_proc_stat_snapshots(s1: &str, s2: &str, elapsed_secs: f64) -> Vec<Signal>` shall be preserved unchanged. Note: `cpu.rs` retains its private `CpuTimes` struct and `parse_cpu_aggregate` helper through Phase 6 (they still serve `from_proc_stat_snapshots`); both are deleted in Phase 7 cleanup when the private `/proc` helpers are removed.

R15. `from_cpu_snapshots()` shall compute the same signals as `from_proc_stat_snapshots()` using `CpuSnapshot` fields. It shall emit `cpu.iowait_pct` only when `a.iowait.is_some() && b.iowait.is_some()`. It shall emit `vmstat.r` only when `b.procs_running.is_some()`. It shall emit `cpu.ctxt_per_sec` only when `a.ctxt.is_some() && b.ctxt.is_some()`.

R16. `NetworkCollector::collect()` shall call `platform::read_net_snapshot()` twice (with `MIN_WINDOW` sleep), then call a new `from_net_snapshots(a: &NetSnapshot, b: &NetSnapshot, elapsed_secs: f64) -> Vec<Signal>`. `net.tw_count` shall be read from snapshot `b` directly (point-in-time, not a delta). The existing public `from_snapshots(dev1: &str, dev2: &str, snmp1: &str, snmp2: &str, elapsed_secs: f64) -> Vec<Signal>` shall be preserved.

R17. `DiskCollector::collect()` shall call `platform::read_disk_snapshots()` twice (with `MIN_WINDOW` sleep), then call a new `from_disk_snapshots(a: &[DiskDevSnapshot], b: &[DiskDevSnapshot], elapsed_secs: f64) -> Vec<Signal>`. Signals `disk.{dev}.util_pct` and `disk.{dev}.await_ms` shall be emitted only when `a_dev.io_time_ms.is_some() && b_dev.io_time_ms.is_some()`. The existing public `from_proc_diskstats_snapshots(s1: &str, s2: &str, elapsed_secs: f64) -> Vec<Signal>` shall be preserved.

R18. `HostCollector::collect()` shall call `platform::read_host_snapshot()` and populate signals from its fields. When `read_host_snapshot()` returns `None`, fall back to `std::thread::available_parallelism()` for `cpu_count` and `0.0` for remaining fields (matching current fallback behaviour). The private `read_mem_total_bytes()` and `read_load_avg_1m()` functions shall be removed from `host.rs` (they move to `linux.rs`).

R19. `CpuFreqCollector::collect()` shall call `platform::read_cpufreq_snapshot()` and emit signals from its fields (`freq_ratio` and `temp_celsius` only when `Some`). The private `read_freq_khz()` and `read_max_temp_celsius()` functions shall be removed from `cpufreq.rs` (they move to `linux.rs`).

R20. `MemoryCollector` shall gain a `pub fn new() -> Self` constructor. The `stdout: String` field shall become `stdout: Option<String>`; `new()` initialises it to `None`; `from_stdout(s: String) -> Self` sets it to `Some(s)`. Its `collect()` implementation shall call `platform::read_mem_snapshot()` on **both** platforms — no `#[cfg(target_os)]` attribute in `collect()` (consistent with R1 and every other collector). The linux platform function runs `free -m` internally (per R7); the macOS platform function runs `vm_stat` and `sysctl vm.swapusage` internally (per R11). A new `pub fn signals_from_mem_snapshot(snap: &MemSnapshot) -> collector::Result<Vec<Signal>>` shall be added to `memory.rs` as the shared snapshot-to-signal conversion function for both platforms; it uses the `Result` type alias already in scope via `use super::Result`. All vm_stat string parsing logic shall live in `platform/macos.rs` with fixture-based unit tests there — no macOS-specific parse functions in `memory.rs`. The existing `from_stdout(stdout: String) -> Self` and the private `parse_free_output(s: &str) -> Result<Vec<Signal>>` helper shall be preserved (the latter is the legacy path used only by `from_stdout`-constructed instances).

R21. `MemoryCollector::new()` shall be added to the collector list in `src/cli/mod.rs`. The import of `MemoryCollector` shall be added to the `use crate::collector::...` block at the top of `cli/mod.rs`.

**No-change collectors**

R22. `interrupts.rs`, `cgroup.rs`, `dmesg.rs`, and `bpf.rs` shall not be modified — they already return empty signals gracefully on macOS.

---

## File & Module Structure

| Path | Action | Description |
|------|--------|-------------|
| `src/collector/platform/mod.rs` | NEW | Snapshot types + sole location of all `#[cfg(target_os)]`; re-exports six platform fns |
| `src/collector/platform/linux.rs` | NEW | 6 platform functions using `/proc`, `/sys`, and `free -m` |
| `src/collector/platform/macos.rs` | NEW | 6 platform functions using `sysctl`, `netstat`, `vm_stat` |
| `src/collector/mod.rs` | EDIT | Add `pub mod platform;` |
| `src/collector/cpu.rs` | EDIT | Add `from_cpu_snapshots()`; update `collect()`; remove `parse_cpu_aggregate`, `parse_procs_running`, `parse_ctxt` (move to `linux.rs`) |
| `src/collector/network.rs` | EDIT | Add `from_net_snapshots()`; update `collect()`; remove `parse_rx_drops`, `parse_tcp_snmp`, `parse_tw_count` (move to `linux.rs`) |
| `src/collector/disk.rs` | EDIT | Add `from_disk_snapshots()`; update `collect()`; remove `parse_diskstats` (moves to `linux.rs`) |
| `src/collector/host.rs` | EDIT | Replace `/proc` helpers with `platform::read_host_snapshot()`; remove `read_mem_total_bytes`, `read_load_avg_1m` |
| `src/collector/cpufreq.rs` | EDIT | Replace `/sys` helpers with `platform::read_cpufreq_snapshot()`; remove `read_freq_khz`, `read_max_temp_celsius` |
| `src/collector/memory.rs` | EDIT | Add `signals_from_mem_snapshot()`, `new()`, update `collect()` (no cfg); keep `from_stdout()`, `parse_free_output()` |
| `src/cli/mod.rs` | EDIT | Import `memory::MemoryCollector`; add `Box::new(MemoryCollector::new())` to collector vec |

---

## Data Models

All types defined in `src/collector/platform/mod.rs`:

```rust
#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: Option<u64>,     // None on macOS
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
    pub procs_running: Option<u64>,
    pub ctxt: Option<u64>,
}

impl CpuSnapshot {
    /// Total tick count across all fields; used for computing percentages.
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
    pub rx_drops: std::collections::HashMap<String, u64>,
    pub tcp_out_segs: u64,
    pub tcp_retrans_segs: u64,
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
    pub available_mb: Option<f64>,  // None on macOS
    pub swap_total_mb: f64,
    pub swap_used_mb: f64,
    pub swap_free_mb: f64,
}

#[derive(Debug, Clone)]
pub struct CpuFreqSnapshot {
    pub freq_ratio: Option<f64>,    // None on macOS
    pub temp_celsius: Option<f64>,  // None on macOS
}

#[derive(Debug, Clone)]
pub struct DiskDevSnapshot {
    pub name: String,
    pub read_ios: u64,
    pub write_ios: u64,
    pub read_time_ms: Option<u64>,  // None on macOS
    pub write_time_ms: Option<u64>, // None on macOS
    pub io_time_ms: Option<u64>,    // None on macOS
}
```

---

## API Contracts

Six platform functions, identical signatures in both `linux.rs` and `macos.rs`:

```rust
/// Returns None if the required source (/proc/stat or kern.cp_time) is
/// unavailable or unparseable.
pub fn read_cpu_snapshot() -> Option<CpuSnapshot>;

/// Returns None if /proc/net/dev (Linux) or netstat (macOS) cannot be read.
/// Individual sub-fields default to 0/empty on parse failure — not None.
pub fn read_net_snapshot() -> Option<NetSnapshot>;

/// Returns None if the primary source (/proc/loadavg or sysctl hw.logicalcpu)
/// cannot be read or parsed.
pub fn read_host_snapshot() -> Option<HostSnapshot>;

/// Returns None if free -m (Linux) or vm_stat (macOS) cannot be run or
/// if page size cannot be parsed from vm_stat header.
pub fn read_mem_snapshot() -> Option<MemSnapshot>;

/// Never returns an error. Returns CpuFreqSnapshot with all fields None
/// when sources are unavailable.
pub fn read_cpufreq_snapshot() -> CpuFreqSnapshot;

/// Never returns an error. Returns Vec::new() when sources are unavailable
/// (always on macOS).
pub fn read_disk_snapshots() -> Vec<DiskDevSnapshot>;
```

`platform/mod.rs` re-exports:

```rust
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
```

---

## Error Handling

| Failure | Trigger | Behaviour | User-visible |
|---------|---------|-----------|--------------|
| `sysctl` binary not in PATH | macOS system without sysctl (impossible in practice) | `read_cpu_snapshot()` returns `None`; collector emits empty `Vec<Signal>` | No signal for that collector; no crash |
| `sysctl` returns non-zero exit | macOS permission or key-not-found | Returns `None` / empty; same as above | No signal |
| `kern.cp_time` output malformed | Unexpected kernel output format | `read_cpu_snapshot()` returns `None` | No signal |
| `vm_stat` page size line missing | Unexpected vm_stat output | `read_mem_snapshot()` returns `None` | No mem signals on macOS |
| `vm.swapusage` unparseable | Unexpected sysctl output | `swap_*` fields default to `0.0`; function still returns `Some` | Swap signals show 0 |
| `netstat -s -p tcp` line not found | Unexpected netstat output | `tcp_out_segs`/`tcp_retrans_segs` default to `0`; `tcp_tw_count` = `None` | No retrans/tw signal |
| `/proc/stat` not present | macOS, FreeBSD | `read_cpu_snapshot()` returns `None` | No cpu signals on that platform |
| `free -m` not in PATH | macOS (no `free` command) | `read_mem_snapshot()` returns `None` | No mem signals |
| `MemoryCollector::new().collect()` on Linux where `free` missing | Container without `free` | Returns `Ok(Vec::new())` | No mem signals; rule silent |

---

## Implementation Phases

### Phase 1 — Platform module skeleton

Create `src/collector/platform/mod.rs` with all six snapshot struct definitions and the `#[cfg]`-gated re-exports. Create `src/collector/platform/linux.rs` and `src/collector/platform/macos.rs` as stubs returning `None` / empty. Add `pub mod platform;` to `src/collector/mod.rs`.

**Commit when:** `cargo build --all-features` succeeds on both Linux and macOS with stubs in place.

### Phase 2 — Linux platform implementation

Fill in `src/collector/platform/linux.rs` with complete implementations of all six functions, copying (not yet removing) parse logic from the existing collector files. Note: `read_mem_snapshot()` in `linux.rs` must implement its own `free -m` stdout → `MemSnapshot` conversion; it cannot call `parse_free_output()` (wrong return type). Tests shall use plain `assert!`/`assert_eq!` (collector convention).

**Commit when:** `cargo test --all-features` passes on Linux; no Linux collector behaviour changes.

### Phase 3 — macOS platform implementation

Fill in `src/collector/platform/macos.rs` with the six macOS implementations (R8–R13). Add unit tests in `macos.rs` using fixture strings for `parse_*` helpers — no real OS calls needed. Tests shall use plain `assert!`/`assert_eq!` macros (collector subtree convention, not `assert_that!` from googletest).

**Commit when:** `cargo test --all-features` passes on macOS; all six functions return non-empty/non-None results on a real macOS host.

### Phase 4 — Refactor host and cpufreq collectors

Update `host.rs` to call `platform::read_host_snapshot()` (R18). Update `cpufreq.rs` to call `platform::read_cpufreq_snapshot()` (R19). Remove the now-dead private helpers from those files.

**Commit when:** `cargo test --all-features` passes on both platforms. `grep -n 'proc/loadavg\|proc/meminfo\|sys/devices\|sys/class/thermal' src/collector/host.rs src/collector/cpufreq.rs` returns nothing.

### Phase 5 — Refactor cpu, network, and disk collectors

Add `from_cpu_snapshots()` to `cpu.rs` and update `collect()` (R14, R15). Add `from_net_snapshots()` to `network.rs` and update `collect()` (R16). Add `from_disk_snapshots()` to `disk.rs` and update `collect()` (R17). Keep existing public `from_proc_stat_snapshots`, `from_snapshots`, `from_proc_diskstats_snapshots`.

**Commit when:** `cargo test --all-features` passes on both platforms. No `#[cfg(target_os)]` in `cpu.rs`, `network.rs`, or `disk.rs`.

### Phase 6 — Refactor memory collector and CLI registration

Add `pub fn new() -> Self` and `pub fn signals_from_mem_snapshot(snap: &MemSnapshot) -> Result<Vec<Signal>>` to `memory.rs`. Update `collect()` to call `platform::read_mem_snapshot()` on both platforms with no `#[cfg]` in `collect()` (R20). Add `MemoryCollector` to the collector list in `cli/mod.rs` (R21).

**Commit when:** `cargo test --all-features` passes on both platforms. On macOS, `usereport --output json` output contains `mem.free_pct` and `mem.total_mb` signals. On Linux, the same signals appear via `MemoryCollector`.

### Phase 7 — Cleanup

Remove private parse helpers from collector files that have been moved to `linux.rs` (the helpers copied in Phase 2). Run `cargo clippy --all-features -- -D warnings` and fix any warnings.

**Commit when:** `cargo clippy --all-features -- -D warnings` is clean on Linux. `grep -rn 'cfg(target_os' src/collector/ | grep -v 'platform/mod.rs'` returns nothing.

---

## Test Scenarios

**T1 — cfg isolation**
GIVEN the fully implemented platform module,
WHEN `grep -rn 'cfg(target_os' src/collector/` is run,
THEN every match is in `src/collector/platform/mod.rs` and nowhere else.

**T2 — iowait absent on macOS**
GIVEN a `CpuSnapshot` with `iowait: None`,
WHEN `from_cpu_snapshots(&a, &b, 1.0)` computes signals,
THEN the returned `Vec<Signal>` contains no signal with id `cpu.iowait_pct`.

**T3 — iowait present on Linux**
GIVEN a `CpuSnapshot` with `iowait: Some(500)` in both `a` and `b`,
WHEN `from_cpu_snapshots(&a, &b, 1.0)` computes signals,
THEN the returned `Vec<Signal>` contains a signal with id `cpu.iowait_pct`.

**T4 — disk util/await absent when time fields are None**
GIVEN `DiskDevSnapshot` entries with `io_time_ms: None`, `read_time_ms: None`, `write_time_ms: None`,
WHEN `from_disk_snapshots(&a, &b, 1.0)` computes signals,
THEN no `disk.*.util_pct` or `disk.*.await_ms` signals are emitted; `disk.*.read_iops` and `disk.*.write_iops` are still emitted.

**T5 — vm_stat parser (in platform/macos.rs)**
GIVEN the fixture string:
```
Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                            12345.
Pages active:                          56789.
Pages inactive:                        11111.
Pages speculative:                      2222.
Pages wired down:                       8888.
Pages purgeable:                        3333.
```
And a `sysctl -n vm.swapusage` fixture: `total = 2048.00M  used = 512.00M  free = 1536.00M`
WHEN the internal vm_stat parse helper in `platform/macos.rs` is called with the fixture,
THEN a `MemSnapshot` is produced where total_pages = 12345+56789+11111+2222+8888+3333 = 94688, total_mb = 94688 * 16384 / 1048576 ≈ 1479.5 MB, free_mb = (12345+2222) * 16384 / 1048576.
WHEN `signals_from_mem_snapshot(&snap)` is called on that snapshot,
THEN `mem.total_mb`, `mem.free_mb`, `mem.used_mb`, `mem.free_pct` signals are present.

**T6 — netstat tcp parser**
GIVEN a `netstat -s -p tcp` fixture string containing:
```
    12345 packets sent
    ...
    678 data packets (9012 bytes) retransmitted
    ...
    99 connections in TIME_WAIT
```
WHEN the macOS `read_net_snapshot()` parse logic for tcp stats is called,
THEN `tcp_out_segs = 12345`, `tcp_retrans_segs = 678`, `tcp_tw_count = Some(99)`.

**T7 — Linux collect() returns ok (regression)**
GIVEN a Linux host with `/proc/stat` present,
WHEN `CpuCollector::new().collect(&CollectCtx::default())` is called,
THEN the result is `Ok(signals)` with `cpu.usr_pct` present.

**T8 — macOS collect() returns ok**
GIVEN a macOS host with `sysctl` in PATH,
WHEN `CpuCollector::new().collect(&CollectCtx::default())` is called,
THEN the result is `Ok(signals)` with `cpu.usr_pct` present and no signal with id `cpu.iowait_pct`.

**T9 — mem signals fire on macOS**
GIVEN a macOS host and `MemoryCollector::new()` registered in the CLI,
WHEN `MemoryCollector::new().collect(&CollectCtx::default())` is called,
THEN the result contains a signal with id `mem.free_pct`.

**T10 — existing tests unchanged**
GIVEN the existing unit tests `from_proc_stat_snapshots` (cpu.rs), `from_snapshots` (network.rs), `from_proc_diskstats_snapshots` (disk.rs),
WHEN `cargo test --all-features` is run on Linux,
THEN all existing tests pass without modification.

**T11 — netstat drop parser skips loopback**
GIVEN a `netstat -i -b -n` fixture with rows for `lo0` and `en0`,
WHEN the macOS `read_net_snapshot()` rx_drops parser runs,
THEN `rx_drops` does not contain a key `lo0` and does contain a key `en0`.

---

## Decision Log

**dtrace rejected** — Modern macOS (Big Sur+) requires SIP partially disabled or process entitlements for system-wide kernel probing. The "no root" constraint rules it out entirely. `sysctl`/`netstat`/`vm_stat` give better coverage without privileges.

**No trait for platform dispatch** — A `PlatformCollector` trait was considered and rejected (YAGNI). Plain `pub fn` re-exported via `pub use` achieves the same single-`#[cfg]` goal without abstraction overhead. Adding a trait would require boxing or generics throughout the collector tree for no gain.

**Single platform/ module vs per-collector split** — The alternative of one `#[cfg]` per collector file (`cpu/linux.rs` + `cpu/macos.rs` etc.) was considered. Rejected because it scatters OS dispatch across 6 files and makes auditing harder. A single `platform/mod.rs` keeps all OS dispatch in one place.

**macOS cpufreq skipped** — Intel Macs expose `sysctl hw.cpufrequency`, but Apple Silicon does not. Detecting the chip type adds complexity for one signal covering one rule. Returns `None` for both `freq_ratio` and `temp_celsius` on macOS.

**macOS disk skipped** — `iostat` on macOS gives throughput and tps but not `util_pct` or `await_ms`, which are the only two disk signals used in rules. Not worth the complexity for zero additional rule coverage.

**MemoryCollector::new() bug fix bundled** — `MemoryCollector` is absent from `cli/mod.rs`, meaning `mem.pressure` never fires even on Linux. This is a bug; R21 fixes it as part of this SDD rather than a separate change.

**memory.rs::collect() is platform-neutral** — An early draft of R20 had `collect()` branch on `#[cfg(target_os)]` to call either `platform::read_mem_snapshot()` or `free -m` directly. This violated R1. Since `platform/linux.rs::read_mem_snapshot()` already runs `free -m` internally (R7), `collect()` calls `platform::read_mem_snapshot()` unconditionally on both platforms.

**parse_vm_stat_output stays in macos.rs** — A draft of R20 proposed a `pub fn parse_vm_stat_output(s: &str)` in `memory.rs` "for testing." Rejected: vm_stat is macOS-specific; the parser belongs in `platform/macos.rs` with fixture tests there. Exposing it from `memory.rs` would place macOS-specific logic in a platform-neutral file.

**procs_running: None on macOS** — Using `vm.loadavg` first float cast to `u64` as a `procs_running` proxy was rejected: load average is a lagged exponential, not an instantaneous runnable count, and `0.52 → 0` would silently suppress the `vmstat.r` rule. Setting `procs_running: None` is consistent with the `iowait: None` precedent and correct per R15 (emit `vmstat.r` only when `procs_running` is `Some`).

**linux.rs uses `free -m` not `/proc/meminfo` directly** — The existing `MemoryCollector` already parses `free -m` output. `platform/linux.rs::read_mem_snapshot()` runs `free -m` and independently converts its output into a `MemSnapshot` (it cannot reuse `parse_free_output`, which returns `Vec<Signal>` — an incompatible type). The private `parse_free_output` remains the legacy path used only by `from_stdout()`-constructed instances. Direct `/proc/meminfo` reading is deferred.

---

## Open Decisions

None.

---

## Out of Scope

- macOS CPU frequency and temperature signals (requires root or chip-specific sysctl detection across Intel/Apple Silicon)
- macOS disk util% and await_ms (not exposed by `iostat` without root)
- macOS IRQ-per-CPU breakdown (`net.max_cpu_irq_pct`) — no `/proc/interrupts` equivalent
- eBPF / dtrace / flamegraph on macOS
- Windows support
- Workload-specific rule packs for macOS
- Direct `/proc/meminfo` parsing for Linux `read_mem_snapshot()` (currently delegates to `free -m`)
