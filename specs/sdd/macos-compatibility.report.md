# SDD Implementation Report: macos-compatibility.md

**Date**: 2026-05-02
**Phases run**: 1, 2, 3, 4, 5, 6, 7
**Overall status**: all-shipped
**Commit**: cc89881

| Phase | Title | Status | Commit |
|-------|-------|--------|--------|
| 1 | Platform module skeleton | shipped | cc89881 |
| 2 | Linux platform implementation | shipped | cc89881 |
| 3 | macOS platform implementation | shipped | cc89881 |
| 4 | Refactor host and cpufreq collectors | shipped | cc89881 |
| 5 | Refactor cpu, network, and disk collectors | shipped | cc89881 |
| 6 | Refactor memory collector and CLI registration | shipped | cc89881 |
| 7 | Cleanup | shipped | cc89881 |

---

## Acceptance Criteria

| # | Criterion | Status |
|---|-----------|--------|
| T1 | cfg isolation — no `cfg(target_os)` outside `platform/mod.rs` in `src/collector/` | passing |
| T2 | iowait absent on macOS (`from_cpu_snapshots` with `iowait:None`) | passing |
| T3 | iowait present on Linux (`from_cpu_snapshots` with `iowait:Some`) | passing |
| T4 | disk util/await absent when `io_time_ms: None` | passing |
| T5 | vm_stat parser produces correct MemSnapshot | passing |
| T6 | netstat tcp stats parser | passing |
| T7 | Linux collect() regression | passing |
| T8 | macOS collect() returns ok | passing |
| T9 | mem signals fire (MemoryCollector::new().collect()) | passing |
| T10 | Existing tests unchanged | passing |
| T11 | netstat drops parser skips lo0 | passing |

---

## Notable Implementation Decisions

**kern.cp_time not available on Apple Silicon** — The SDD specified `sysctl -n kern.cp_time` for macOS CPU ticks, but this BSD sysctl does not exist on Apple Silicon macOS (M1/M2/M3/M4). The implementation falls back to `/usr/sbin/iostat 1 1` which gives one-second CPU percentages. These are converted to cumulative tick estimates using `uptime_secs * HZ * ncpus * pct/100` so that two snapshots taken seconds apart have a non-zero delta for the delta engine.

**All 18 test suites pass**, including all new SDD tests and all pre-existing regression tests.

---

## Manual Test Plan

```sh
# 1. Verify cfg isolation
grep -rn 'cfg(target_os' src/collector/ | grep -v 'platform/mod.rs'
# expected: no output

# 2. Verify macOS signals on macOS host
cargo run --all-features -- --output json 2>/dev/null | jq '[.signals[].id] | sort | unique'
# expected: includes "cpu.usr_pct", "host.cpu_count", "mem.free_pct", "net.rx_drops"

# 3. Verify iowait absent on macOS
cargo run --all-features -- --output json 2>/dev/null | jq '[.signals[].id] | map(select(. == "cpu.iowait_pct"))'
# expected: []

# 4. Verify MemoryCollector registered
cargo run --all-features -- --output json 2>/dev/null | jq '[.signals[].id] | map(select(startswith("mem.")))'
# expected: includes "mem.free_pct", "mem.total_mb", etc.

# 5. Run full test suite
cargo test --all-features
# expected: all pass
```
