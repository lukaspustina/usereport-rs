# usereport

[![CI](https://github.com/lukaspustina/usereport-rs/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/lukaspustina/usereport-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/usereport-rs.svg)](https://crates.io/crates/usereport-rs)
[![docs.rs](https://docs.rs/usereport-rs/badge.svg)](https://docs.rs/crate/usereport-rs/)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

> Your server is on fire. You have 60 seconds. Go.

`usereport` is the tool you reach for when a server misbehaves and you need answers _now_. It runs a curated set of performance analysis commands in parallel, reads kernel signals directly from `/proc` and `/sys` on Linux (or uses native commands on macOS), evaluates a rule engine against everything it finds, and hands you a structured report — in Markdown, HTML, JSON, or a format you define yourself.

It follows Brendan Gregg's [USE methodology](http://www.brendangregg.com/usemethod.html): **Utilization, Saturation, Errors** — the fastest path from "something is wrong" to "here is what and why."

<p align="center">
  <a href="docs/linux-net-usereport-html-1.jpg"><img src="https://raw.githubusercontent.com/lukaspustina/usereport-rs/master/docs/linux-net-usereport-html-1.jpg" width="48%" /></a>
  <a href="docs/linux-net-usereport-html-2.jpg"><img src="https://raw.githubusercontent.com/lukaspustina/usereport-rs/master/docs/linux-net-usereport-html-2.jpg" width="48%" /></a>
</p>

---

## What it does in 60 seconds

```
$ usereport --profile net --output html -O report.html
```

While that renders, `usereport` has already:

- Run vmstat, netstat, ss, ethtool, and friends **in parallel** (vm_stat, netstat, nettop on macOS)
- Read `/proc/net/dev`, `/proc/interrupts`, `/proc/net/snmp` directly — no tool required _(Linux)_
- Checked CPU frequency throttling, thermal zones, cgroup memory limits _(Linux)_
- Correlated signals against 15+ built-in rules (retransmits, TIME_WAIT exhaustion, IRQ imbalance, …)
- Matched multi-signal patterns (lock contention, thundering herd, socket leak, …)
- Flagged anomalies against your recorded baseline
- Printed everything in a single self-contained HTML file

No daemons. No agents. No cloud. One binary, one command.

---

## Contents

- [Demo](#demo)
  - [Findings in Markdown](#findings-in-markdown-default-output)
  - [Signals and findings as JSON](#signals-and-findings-as-json)
  - [Exit code in CI](#exit-code-in-ci)
  - [Explain any finding or signal](#explain-any-finding-or-signal)
  - [LLM-ready output with redaction](#llm-ready-output-with-redaction)
  - [Baseline comparison](#baseline-comparison)
  - [Diff two runs](#diff-two-runs)
- [Real-world scenarios](#real-world-scenarios)
  - [Scenario A — TIME_WAIT exhaustion on a busy API gateway](#scenario-a--time_wait-exhaustion-on-a-busy-api-gateway)
  - [Scenario B — OOM kills taking down a Java service](#scenario-b--oom-kills-taking-down-a-java-service)
  - [Scenario C — CPU thermal throttling silently killing a batch job](#scenario-c--cpu-thermal-throttling-silently-killing-a-batch-job)
- [Features](#features)
  - [Direct kernel signal collection](#direct-kernel-signal-collection-_linux_)
  - [Rule engine with a predicate DSL](#rule-engine-with-a-predicate-dsl)
  - [Pattern correlator](#pattern-correlator)
  - [Baselines and drift detection](#baselines-and-drift-detection)
  - [Workload-aware rules](#workload-aware-rules)
  - [eBPF collectors](#ebpf-collectors-opt-in-linux)
  - [CPU flamegraph, inline](#cpu-flamegraph-inline-_linux_)
  - [LLM-ready output](#llm-ready-output)
  - [`explain`](#explain--know-what-youre-looking-at)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Output formats](#output-formats)
- [Configuration](#configuration)
- [Exit codes](#exit-codes)
- [Baselines and drift](#baselines-and-drift)
- [As a library](#as-a-library)
- [Contributing](#contributing)

---

## Demo

### Findings in Markdown (default output)

```
$ usereport

# Use Report - myhost

## Summary
- Host: `myhost`
- Kernel: `Linux 6.8.0-51-generic x86_64`
- Top concern: **Warn** — Free memory below 10% of total.

## Findings

### [Warn] mem.pressure
Free memory below 10% of total.

Evidence:
- `mem.free_pct` = 0.69
- `mem.free_mb` = 131.0 MB
- `mem.total_mb` = 18893 MB

Next steps:
- `ps -eo pid,rss,cmd --sort=-rss | head -20`
- `cat /proc/meminfo`
```

### Signals and findings as JSON

```sh
$ usereport --output json | jq '{signals: [.signals[].id], findings: [.findings[].id]}'
```

```json
{
  "signals": [
    "host.cpu_count", "host.mem_total_bytes", "host.load_avg_1m",
    "cpu.usr_pct", "cpu.sys_pct", "cpu.idle_pct",
    "net.rx_drops", "net.retrans_pct",
    "mem.total_mb", "mem.used_mb", "mem.free_mb", "mem.free_pct",
    "swap.total_mb", "swap.used_mb", "swap.free_mb"
  ],
  "findings": ["mem.pressure"]
}
```

### Exit code in CI

```sh
$ usereport --exit-on warn; echo "exit: $?"
exit: 1   # mem.pressure (Warn) fired

$ usereport --exit-on crit; echo "exit: $?"
exit: 0   # no Crit findings
```

### Explain any finding or signal

```sh
$ usereport explain mem.pressure

ID:       mem.pressure
Severity: Warn
Summary:  Free memory below 10% of total.

To investigate:
  ps -eo pid,rss,cmd --sort=-rss | head -20
  cat /proc/meminfo
```

### LLM-ready output with redaction

```sh
$ usereport --output llm --redact | jq '{schema_version, hostname: .host.hostname, findings: [.findings[].id]}'
```

```json
{
  "schema_version": "1",
  "hostname": "66b11e02cb5b987d50fca84ad73670c4",
  "findings": ["mem.pressure"]
}
```

The hostname (and any IPs or MACs) is replaced with a stable HMAC hash — same host always produces the same hash, but nothing leaves the machine in plaintext.

### Baseline comparison

```sh
# Record a clean snapshot
$ usereport baseline record --name green

# Later, compare — anomalies (|z| > 3) become automatic findings
$ usereport --baseline green --output json | jq '.signals[] | select(.baseline != null) | {id, z_score: .baseline.z_score}'
```

### Diff two runs

```sh
$ usereport --output json > before.json
# ... reproduce the incident ...
$ usereport --output json > after.json
$ usereport diff before.json after.json
```

---

## Real-world scenarios

### Scenario A — TIME_WAIT exhaustion on a busy API gateway

**Symptom:** A Node.js reverse proxy starts refusing new connections under load. Application logs show `ECONNREFUSED` on outgoing calls. CPU and memory look fine.

```sh
$ usereport --output markdown
```

```
## Summary
- Host: `api-gw-03`
- Top concern: **Crit** — TIME_WAIT exhaustion likely: high tw_count with active connect failures.

## Findings

### [Crit] time_wait_exhaustion   ← pattern: two signals correlated
TIME_WAIT exhaustion likely: high tw_count with active connect failures.

Next steps:
- sysctl net.ipv4.tcp_tw_reuse
- sysctl net.ipv4.ip_local_port_range
- ss -s

### [Warn] net.time_wait_high
TIME_WAIT socket count above 28k — port exhaustion risk.

Evidence:
- `net.tw_count` = 31847

Next steps:
- ss -tan state time-wait | wc -l
- sysctl net.ipv4.tcp_tw_reuse

### [Warn] net.retransmit_elevated
TCP retransmission rate above 1%.

Evidence:
- `net.retrans_pct` = 2.3
```

The `time_wait_exhaustion` pattern fired because both `net.tw_count > 28000` **and** `net.connect_failures > 0` are true simultaneously — the kernel is accumulating TIME_WAIT sockets faster than the ephemeral port range can recycle them, and new `connect()` calls are failing with `EADDRNOTAVAIL`.

**Investigate:**

```sh
# Confirm: how many ephemeral ports are actually available?
sysctl net.ipv4.ip_local_port_range       # default: 32768–60999 = ~28k ports
ss -tan state time-wait | wc -l           # live TIME_WAIT count

# Which service is holding them?
ss -tan state time-wait | awk '{print $5}' | cut -d: -f1 | sort | uniq -c | sort -rn
```

**Fix:**

```sh
# Allow TIME_WAIT sockets to be reused for new outgoing connections
sysctl -w net.ipv4.tcp_tw_reuse=1

# Widen the ephemeral port range from ~28k to ~64k ports
sysctl -w net.ipv4.ip_local_port_range="1024 65535"

# Persist across reboots
echo "net.ipv4.tcp_tw_reuse = 1" >> /etc/sysctl.conf
echo "net.ipv4.ip_local_port_range = 1024 65535" >> /etc/sysctl.conf
```

**Verify the fix:**

```sh
$ usereport --output json > before.json   # captured before the fix above
$ usereport --output json > after.json    # run after applying sysctls
$ usereport diff before.json after.json
```

```
Signals changed:
  net.tw_count         31847  →    412   (-31435)
  net.connect_failures   143  →      0   (-143)
  net.retrans_pct        2.3  →    0.1   (-2.2)

Findings only in before.json:
  [Crit] time_wait_exhaustion
  [Warn] net.time_wait_high
  [Warn] net.retransmit_elevated

Findings only in after.json:
  (none)
```

---

### Scenario B — OOM kills taking down a Java service

**Symptom:** A JVM microservice restarts randomly every few hours. Heap dumps are truncated. The service gets progressively slower before each crash.

```sh
$ usereport --output markdown
```

```
## Summary
- Host: `svc-worker-07`
- Top concern: **Crit** — Out-of-memory killer fired since the last log rotate.

## Findings

### [Crit] dmesg.oom_kill
Out-of-memory killer fired since the last log rotate.

Evidence:
- `dmesg.oom_count` = 2

Next steps:
- dmesg -T | grep -i 'killed process'
- journalctl -k --since -1h

### [Warn] mem.pressure
Free memory below 10% of total.

Evidence:
- `mem.free_pct`  = 3.2
- `mem.free_mb`   = 262
- `mem.total_mb`  = 8192

Next steps:
- ps -eo pid,rss,cmd --sort=-rss | head -20
- cat /proc/meminfo

### [Warn] mem.swap_in_active
Pages are being swapped in from disk.

Evidence:
- `vmstat.swap_in` = 847
```

Two findings tell the full story: the OOM killer has already fired twice (`dmesg.oom_count = 2`), and the host is actively paging (`vmstat.swap_in = 847` pages swapped in during the 1-second measurement window). The service is thrashing swap before the kernel kills it.

**Investigate:**

```sh
# Who did the OOM killer take out, and why?
dmesg -T | grep -i 'killed process'
# [Tue May  6 03:17:42 2025] Out of memory: Killed process 18423 (java) \
#   total-vm:12582912kB, anon-rss:7340032kB, file-rss:0kB

# What is the JVM heap configured to?
cat /proc/18423/cmdline | tr '\0' '\n' | grep -i xmx
# (process may be gone — check the service unit instead)
systemctl cat svc-worker | grep -i xmx
# ExecStart=/usr/bin/java -Xmx6g -jar /opt/svc-worker.jar

# What is the host's total RAM?
grep MemTotal /proc/meminfo
# MemTotal: 8388608 kB  →  8 GB total; -Xmx6g leaves only 2 GB for OS + JVM overhead

# Feed to LLM for root cause analysis
usereport --output llm --redact | your-ai-cli "what is killing this service?"
```

**Fix:**

The JVM heap ceiling (`-Xmx6g`) plus JVM overhead, OS, and other processes exceeds available RAM. Either reduce the heap or add memory.

```sh
# Option 1: reduce heap to leave headroom
# Edit service unit: -Xmx6g  →  -Xmx5g
systemctl edit svc-worker
systemctl restart svc-worker

# Option 2: add swap as a temporary buffer while arranging more RAM
fallocate -l 4G /swapfile && chmod 600 /swapfile
mkswap /swapfile && swapon /swapfile
```

**Record a baseline once stable, detect regression early:**

```sh
usereport baseline record --name stable

# Next day, after a deployment:
usereport --baseline stable --exit-on warn
# Exits 1 if mem.free_pct drops below 10% again — use in post-deploy health check
```

---

### Scenario C — CPU thermal throttling silently killing a batch job

**Symptom:** A nightly data-processing job that normally completes in 20 minutes is taking 90 minutes. No code was deployed. CPU utilization reported by monitoring looks normal. The team is baffled.

```sh
$ usereport --baseline last-tuesday --output markdown
```

```
## Summary
- Host: `batch-proc-02`
- Top concern: **Warn** — CPU frequency below 80% of nominal — throttled.

## Findings

### [Warn] cpu.frequency_throttling
CPU frequency below 80% of nominal — throttled.

Evidence:
- `cpu.freq_ratio` = 0.41

Next steps:
- sensors
- cat /sys/class/thermal/thermal_zone*/temp

### [Warn] Anomaly: cpu.freq_ratio  ← automatic baseline outlier finding
Signal cpu.freq_ratio deviates from baseline (z_score = 28.5).
Baseline p50 = 0.98, observed = 0.41.
```

The paradox is now visible: `cpu.idle_pct` is around 60% (the CPU has capacity), yet the job takes 4.5× longer. The CPU is throttling to **41% of its rated frequency** due to thermal protection — running slowly not because it's busy, but because it's hot. The baseline comparison makes this unmissable: `cpu.freq_ratio` normally sits at 0.98 and is now at 0.41, a z-score of 28.5.

**Investigate:**

```sh
# Current frequency vs. maximum across all cores
paste \
  <(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq) \
  <(cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_max_freq) | \
  awk '{printf "core: %d MHz / %d MHz (%.0f%%)\n", $1/1000, $2/1000, $1/$2*100}'

# Temperature of all thermal zones
paste \
  <(ls /sys/class/thermal/thermal_zone*/type | xargs -I{} cat {}) \
  <(cat /sys/class/thermal/thermal_zone*/temp | awk '{printf "%.1f°C\n", $1/1000}')
# x86_pkg_temp    94.0°C   ← approaching TjMax (usually 95–105°C)
# acpitz          91.0°C

# Has the CPU been throttling long? Check frequency history via turbostat (if available)
turbostat --show Busy,Avg_MHz,TSC_MHz --interval 5
```

At 94 °C the processor is one degree below its thermal shutdown point. It has been reducing clock speed to shed heat — the exact behavior `cpu.freq_ratio` captures.

**Fix:**

```sh
# Immediate: reduce workload or lower CPU performance state to cool down
cpupower frequency-set -g powersave   # temporary; buys time

# Investigate the physical cause
# Common culprits in datacenters:
#   - Clogged air filters (check and replace)
#   - Failed chassis fan (listen for missing fan noise; check IPMI)
#   - Thermal paste dried out (multi-year-old server)
#   - Hot aisle containment failure

# Check fan and sensor state via IPMI (if available)
ipmitool sdr type Fan
ipmitool sdr type Temperature
```

Once the cooling issue is resolved (in this case a failed chassis fan replaced by datacenter ops), verify the batch job returns to normal:

```sh
$ usereport --baseline last-tuesday --output json > after-fix.json
$ usereport diff before-fix.json after-fix.json
```

```
Signals changed:
  cpu.freq_ratio     0.41  →  0.97   (+0.56)
  cpu.temp_celsius  94.0   →  61.0   (-33.0)

Findings only in before-fix.json:
  [Warn] cpu.frequency_throttling
  [Warn] Anomaly: cpu.freq_ratio (z_score 28.5)

Findings only in after-fix.json:
  (none)
```

The batch job completed in 22 minutes on the next run.

---

## Features

### Direct kernel signal collection _(Linux)_

On Linux, `usereport` reads the kernel directly for the signals that matter most, with no tool dependency:

| Signal | Source |
|--------|--------|
| `cpu.usr_pct`, `cpu.iowait_pct`, `vmstat.r` | `/proc/stat` |
| `disk.max_util_pct`, `disk.max_await_ms` (per-device: `disk.<dev>.util_pct`, …) | `/proc/diskstats` |
| `net.rx_drops`, `net.retrans_pct`, `net.tw_count` | `/proc/net/dev` + `/proc/net/snmp` + `/proc/net/sockstat` |
| `net.max_cpu_irq_pct` | `/proc/interrupts` |
| `cpu.freq_ratio`, `cpu.temp_celsius` | `/sys/devices/system/cpu/*/cpufreq/` + thermal zones |
| `cgroup.memory_bytes`, `cgroup.oom_kills`, `cgroup.pids_current` | cgroup v1 / v2 auto-detected |

### Rule engine with a predicate DSL

Built-in rules fire findings when signals cross thresholds. Write your own in TOML:

```toml
[[rule]]
id        = "net.retrans_high"
when      = "net.retrans_pct > 1"
severity  = "warn"
summary   = "TCP retransmit rate is elevated (> 1%)"
evidence  = ["net.retrans_pct"]
suggest   = ["ss -tin", "netstat -s | grep retransmit"]
description = "Sustained retransmits indicate congestion, packet loss, or a broken path."
links     = ["https://www.brendangregg.com/perf.html"]
```

Predicates support percentiles (`.p50`, `.p95`, `.p99`), trends (`.trend == "rising"`), cross-signal comparisons, AND/OR logic, and z-score anomaly detection against baselines.

### Pattern correlator

Single-signal rules are fast. Multi-signal patterns catch the subtle stuff:

| Pattern | Signals it correlates |
|---------|-----------------------|
| `time_wait_exhaustion` | `net.tw_count` + `net.connect_failures` |
| `lock_contention` | CPU saturation + high iowait + run-queue depth |
| `thundering_herd` | burst of short-lived processes + CPU spikes |
| `socket_leak` | rising `net.tw_count` without matching traffic |
| `nfs_stall` | iowait spike + NFS mount activity |
| `slab_leak` | rising kernel slab usage over time |

### Baselines and drift detection

Record a healthy snapshot. Every future run is compared against it automatically.

```sh
# Capture a baseline on a healthy Tuesday
usereport baseline record --name tuesday

# Next Friday at 3am when alerts fire:
usereport --baseline tuesday --output html -O incident.html
```

Signals that deviate more than 3 standard deviations get a `warn` finding. More than 6 get `crit`.

### Workload-aware rules

Load a rule pack tuned for what's actually running:

```sh
usereport --workload postgres   # connection saturation, cache hit rate, lock waits
usereport --workload nginx      # connection count, error rate, accept queue depth
usereport --workload java       # GC pressure, heap saturation, thread count
usereport --workload kubelet    # pod count, evictions, image pull latency
```

### eBPF collectors (opt-in, Linux)

When you need to go deeper:

```sh
usereport --bpf   # runqlat, biolatency, tcpretrans, execsnoop, cachestat
```

Emits histogram signals with full percentile stats. Falls back gracefully — if a tool isn't installed, you get an `info` finding instead of an error.

### CPU flamegraph, inline _(Linux)_

```sh
usereport --profile-cpu 30s --output html -O report.html
```

Runs `perf record` for 30 seconds, folds the stacks with [inferno](https://github.com/jonhoo/inferno), and embeds the SVG directly in the HTML report. No extra steps. No separate files. Falls back to an `info` finding when `perf` isn't available.

### LLM-ready output

```sh
usereport --output llm | your-ai-cli "diagnose this"
```

Produces a compact JSON document — signals, findings, checked-ok list, raw excerpts — structured for feeding to an LLM without token waste. Add `--redact` to HMAC-hash hostnames, IPs, and MACs before they leave the machine.

### `explain` — know what you're looking at

```sh
$ usereport explain net.retrans_high

ID:       net.retrans_high
Severity: Warn
Summary:  TCP retransmit rate is elevated (> 1%)

Sustained retransmits indicate congestion, packet loss, or a broken path.

To investigate:
  ss -tin
  netstat -s | grep retransmit

Links:
  https://www.brendangregg.com/perf.html
```

No more "what does this finding mean?" moments at 3am.

---

## Installation

### Pre-built binary (fastest)

```sh
cargo binstall usereport-rs
```

Or grab a binary from the [Releases page](https://github.com/lukaspustina/usereport-rs/releases).

### From source

```sh
cargo install --all-features usereport-rs
```

Requires Rust 1.85+. Install via [rustup](https://rustup.rs) if needed.

---

## Quick start

```sh
# Run the default profile, get Markdown
usereport

# Network investigation, HTML output
usereport --profile net --output html -O net-report.html

# Time-sampled CPU analysis (11 samples over 10s, every 2s after that)
usereport --duration 10s --interval 2s --output json | jq '.findings'

# Postgres server, with baseline comparison
usereport --workload postgres --baseline prod-healthy --exit-on warn

# Deep-dive with eBPF + flamegraph (requires root + perf/bpfcc-tools)
sudo usereport --bpf --profile-cpu 30s --output html -O deep.html
```

---

## Output formats

| Format | Flag | Use it when |
|--------|------|-------------|
| Markdown | `--output markdown` (default) | Terminal reading, pasting into tickets |
| HTML | `--output html` | Sharing reports, flamegraph embedding |
| JSON | `--output json` | Automation, dashboards, `jq` pipelines |
| LLM | `--output llm` | Feeding an AI for diagnosis |
| Custom | `--output template --output-template my.j2` | Your own Jinja2 template |

---

## Configuration

`usereport` ships with built-in configs for Linux and macOS. Override with `--config`:

```sh
usereport --config /etc/usereport/custom.toml
```

### Profiles

Profiles let you run a focused subset of commands. On Linux:

```sh
usereport --show-profiles        # list available profiles
usereport --profile mem          # virtual memory focus
usereport --profile net          # network focus
usereport --profile cpu          # CPU focus
usereport +mpstat -vmstat        # add/remove individual commands
```

### Custom rules

Drop TOML files in `~/.config/usereport/rules.d/`. They merge with the built-ins. A broken file emits a `warn` finding and is skipped — it never breaks the run.

---

## Exit codes

Useful for automation and alerting:

```sh
usereport --exit-on warn   # exit 1 if any warn or crit findings
usereport --exit-on crit   # exit 1 only for crit findings
usereport --exit-on never  # always exit 0 (default)
```

```sh
# In a cron job or CI check:
usereport --exit-on warn && echo "healthy" || pagerduty-alert
```

---

## Baselines and drift

```sh
# Record
usereport baseline record --name prod-$(date +%Y%m%d)

# List
usereport baseline list

# Compare two JSON reports
usereport diff before.json after.json
```

---

## As a library

The core is a published Rust library. Use it to embed signal collection and rule evaluation in your own tools:

```toml
[dependencies]
usereport-rs = "0.2"
```

```rust
use usereport::{Analysis, Context, collector::cpu::CpuCollector, rule::RuleEngine};
```

---

## Contributing

Pull requests and issue reports are welcome. Run `make ci` before pushing — it covers fmt, clippy, tests, audit, deny, and unused-dependency checks in one shot.

```sh
make ci       # full pipeline
make pre-push # lighter: fmt-check + clippy + test
```

---

## Postcardware

`usereport` is MIT-licensed and free. If it saves your skin during an incident, I'd love a postcard from your city:

```
Lukas Pustina
CenterDevice GmbH
Rheinwerkallee 3
53227 Bonn
Germany
```
