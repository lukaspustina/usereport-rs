# Analysis: memory-pressure

## TL;DR
The OOM killer has fired 3 times; free memory is at 2.1% and the system is critically memory-starved.

## Ranked Root Cause
Active memory exhaustion causing OOM kills. [Finding: dmesg.oom_kill] [Finding: mem.low_free]

- Free memory: 2.1% of 64 GiB [Signal: mem.free_pct]
- OOM kill count: 3 [Signal: dmesg.oom_count]
- Victim process: postgres (seen in raw_excerpts)
- Elevated iowait (18.5%) consistent with swap thrashing [Signal: cpu.iowait_pct]

## Alternative Hypotheses
1. **Kernel slab leak** — memory draining without OOM, but OOM events confirm user-space allocation is exhausting memory.
   Disambiguate: check `/proc/slabinfo` for growing slab entries.
2. **Memory mapped files** — mmap'd files consuming anonymous memory.
   Disambiguate: `smaps_rollup` for the postgres process.

## Ordered Next-Step Commands
1. `dmesg -T | grep -i 'killed process'` [Finding: dmesg.oom_kill]
2. `journalctl -k --since -1h` [Finding: dmesg.oom_kill]
3. `free -m` [Finding: mem.low_free]
4. `vmstat -s` [Finding: mem.low_free]
5. `ps aux --sort=-%mem | head -20` [Finding: mem.low_free]

## Ruled Out
- CPU saturation as primary cause: usr_pct=28% elevated but secondary to memory [Signal: cpu.usr_pct]
