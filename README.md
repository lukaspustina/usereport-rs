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

## Features

### Direct kernel signal collection _(Linux)_

On Linux, `usereport` reads the kernel directly for the signals that matter most, with no tool dependency:

| Signal | Source |
|--------|--------|
| `cpu.usr_pct`, `cpu.iowait_pct`, `cpu.runqueue` | `/proc/stat` |
| `disk.util_pct`, `disk.await_ms` | `/proc/diskstats` |
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
usereport --output json | usereport baseline record --name tuesday

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
