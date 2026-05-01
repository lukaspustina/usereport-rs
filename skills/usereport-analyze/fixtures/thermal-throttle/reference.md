# Analysis: thermal-throttle

## TL;DR
CPU is running at 61% of nominal frequency due to thermal throttling, degrading throughput despite a high-CPU workload.

## Ranked Root Cause
Thermal throttling limiting CPU performance. [Finding: cpu.freq_throttled]

- CPU frequency ratio: 0.61 (threshold <0.8) [Signal: cpu.freq_ratio]
- CPU user time: 78.4% [Signal: cpu.usr_pct]
- Run queue: 9 (> cpu_count=8) [Signal: vmstat.r]
- The throttling is turning a CPU-bound workload into effective saturation

## Alternative Hypotheses
1. **Power cap / RAPL limit** — not thermal but power budget throttling.
   Disambiguate: `turbostat --show PkgTmp,PkgWatt` to compare package temperature vs. power.
2. **BIOS power profile** — balanced/powersave governor active.
   Disambiguate: `cpupower frequency-info` to check current governor.

## Ordered Next-Step Commands
1. `cat /sys/class/thermal/thermal_zone*/temp` [Finding: cpu.freq_throttled]
2. `cpupower frequency-info` [Finding: cpu.freq_throttled]
3. `turbostat --show Busy,Avg_MHz,Bzy_MHz,PkgTmp 1 5` [Finding: cpu.freq_throttled]

## Ruled Out
- I/O bottleneck: iowait_pct=0.8% [Signal: cpu.iowait_pct]
- Memory pressure: free_pct=35% [Signal: mem.free_pct]
