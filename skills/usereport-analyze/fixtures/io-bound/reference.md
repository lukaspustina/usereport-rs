# Analysis: io-bound

## TL;DR
Disk sda is fully saturated at 98.2% utilization with 145ms average await, causing system-wide iowait of 42%.

## Ranked Root Cause
Disk I/O saturation on sda. [Finding: disk.sda.saturated] [Finding: disk.sda.high_await]

- Disk utilization: 98.2% [Signal: disk.sda.util_pct]
- Disk await: 145.6ms (threshold >100ms) [Signal: disk.sda.await_ms]
- iowait: 42.3% [Signal: cpu.iowait_pct]
- Read IOPS: 1820 [Signal: disk.sda.read_iops]

## Alternative Hypotheses
1. **Disk hardware failure** — elevated await can indicate bad sectors or failing drive.
   Disambiguate: `smartctl -a /dev/sda` for SMART errors [Finding: disk.sda.high_await].
2. **Read-heavy workload without page cache** — cold cache after restart or large dataset scan.
   Disambiguate: check `vmstat -s` for cache usage; re-run after warm-up.

## Ordered Next-Step Commands
1. `iostat -xz 1 10` [Finding: disk.sda.saturated]
2. `iotop -o` [Finding: disk.sda.saturated]
3. `smartctl -a /dev/sda` [Finding: disk.sda.high_await]

## Ruled Out
- Memory pressure: free_pct=48% [Signal: mem.free_pct]
- CPU saturation: idle_pct=35.1%; workload is I/O-bound, not CPU-bound [Signal: cpu.idle_pct]
