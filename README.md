# usereport

[![CI](https://github.com/lukaspustina/usereport-rs/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/lukaspustina/usereport-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/usereport-rs.svg)](https://crates.io/crates/usereport-rs)
[![docs.rs](https://img.shields.io/docsrs/usereport-rs/latest)](https://docs.rs/usereport-rs)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

> Your server is on fire. You have 60 seconds. Go. -- LLM-ready

`usereport` collects system signals in parallel — from `/proc`, `/sys`, and a curated set of commands — evaluates a rule engine against them, and hands you a structured performance report. One binary, one command, no daemons.

It follows Brendan Gregg's [USE methodology](http://www.brendangregg.com/usemethod.html) — **Utilization, Saturation, Errors** — the fastest path from "something is wrong" to "here is exactly what and why."

<p align="center">
  <a href="docs/linux-net-usereport-html-1.jpg"><img src="https://raw.githubusercontent.com/lukaspustina/usereport-rs/master/docs/linux-net-usereport-html-1.jpg" width="48%" /></a>
  <a href="docs/linux-net-usereport-html-2.jpg"><img src="https://raw.githubusercontent.com/lukaspustina/usereport-rs/master/docs/linux-net-usereport-html-2.jpg" width="48%" /></a>
</p>

```sh
cargo binstall usereport-rs                                        # install

# On the burning server — capture everything in 60 seconds
usereport --output json -O incident.json                           # structured snapshot

# Back at your desk — render and diagnose without touching the server again
usereport convert incident.json --output html -O report.html       # share with your team
usereport convert incident.json --output llm --redact \
  | your-ai "diagnose this and suggest fixes"                      # let the LLM drive
```

---

## What happens in one run

```sh
usereport --profile net --output html -O report.html
```

By the time that command returns, `usereport` has:

- Run vmstat, netstat, ss, ethtool, and friends **in parallel**, with per-command progress spinners
- Read `/proc/net/dev`, `/proc/interrupts`, `/proc/net/snmp` directly — no tool required _(Linux)_
- Checked CPU frequency throttling, thermal zones, and cgroup memory limits _(Linux)_
- Collected memory, network, and CPU stats via native sysctl and vm_stat _(macOS)_
- Evaluated 15+ built-in rules (retransmits, TIME_WAIT exhaustion, IRQ imbalance, …)
- Matched multi-signal patterns that single rules can't catch (lock contention, socket leak, …)
- Compared every signal against your recorded baseline and flagged statistical outliers
- Linked every finding back to the exact command output that triggered it
- Rendered a vital-signs overview and a Coverage Gaps section showing blind spots in your USE coverage
- Written a single self-contained HTML file — no assets, no server required

---

## Why usereport

Most monitoring tools run continuously, need agents, and show you dashboards you interpret yourself. `usereport` does the opposite:

| | usereport | htop / top | sar / sysstat | Datadog / Grafana |
|---|:---:|:---:|:---:|:---:|
| No daemons or agents | **yes** | yes | yes | no |
| Direct `/proc` reads — no tool deps for core signals | **yes** | partial | no | no |
| Rule engine with cross-signal predicate DSL | **yes** | no | no | partial |
| Multi-signal pattern correlation | **yes** | no | no | partial |
| Statistical baseline drift detection | **yes** | no | no | yes |
| Self-contained HTML report with embedded flamegraph | **yes** | no | no | no |
| LLM-ready output with hostname/IP redaction | **yes** | no | no | no |
| Works fully offline and air-gapped | **yes** | yes | yes | no |

The core insight: a `cpu.freq_ratio` of 0.41 only looks wrong if you know it's usually 0.98. A TIME_WAIT alert only fires when `net.tw_count` and `net.connect_failures` are elevated _at the same time_. Thresholds alone can't catch either. `usereport` can.

---

## Contents

- [Quick start](#quick-start)
- [Demo](#demo)
- [Real-world scenarios](#real-world-scenarios)
- [Features](#features)
- [Platform support](#platform-support)
- [Installation](#installation)
- [Output formats](#output-formats)
- [Convert: re-render a saved report](#convert-re-render-a-saved-report)
- [Configuration](#configuration)
- [Exit codes](#exit-codes)
- [Baselines and drift](#baselines-and-drift)
- [As a library](#as-a-library)
- [Contributing](#contributing)

---

## Quick start

```sh
# Default profile — Markdown to stdout
usereport

# Network investigation — HTML report
usereport --profile net --output html -O net-report.html

# CPU deep-dive with time-series sampling
usereport --duration 10s --interval 2s --profile cpu --output json | jq '.findings'

# Postgres host with baseline comparison and CI gate
usereport --workload postgres --baseline prod-healthy --exit-on warn

# Everything, including eBPF histograms and an inline flamegraph
sudo usereport --bpf --profile-cpu 30s --output html -O deep.html

# Verify all required tools are installed on this host
usereport check
```

---

## Demo

### Default output — Markdown

Running bare `usereport` gives a terminal-rendered Markdown report, colored by severity:

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
usereport --output json | jq '{signals: [.signals[].id], findings: [.findings[].id]}'
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

### Exit codes for CI and alerting

```sh
$ usereport --exit-on warn; echo "exit: $?"
exit: 1   # mem.pressure (Warn) fired

$ usereport --exit-on crit; echo "exit: $?"
exit: 0   # no Crit findings

# Drop into a cron job or post-deploy health check:
usereport --exit-on warn && echo "healthy" || pagerduty-alert
```

### Explain any finding or rule

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

### LLM-ready output with redaction

```sh
usereport --output llm --redact | jq '{schema_version, hostname: .host.hostname, findings: [.findings[].id]}'
```

```json
{
  "schema_version": "1",
  "hostname": "66b11e02cb5b987d50fca84ad73670c4",
  "findings": ["mem.pressure"]
}
```

The hostname, all IPs, and MAC addresses are replaced with stable HMAC hashes — same host always produces the same hash, but nothing sensitive leaves the machine in plaintext. Pipe directly to your AI of choice:

```sh
usereport --output llm --redact | your-ai-cli "diagnose this and suggest fixes"
```

### Baseline comparison and diff

```sh
# Record a clean snapshot
usereport baseline record --name green

# Later — anomalies (|z| > 3) become automatic findings
usereport --baseline green --output json \
  | jq '.signals[] | select(.baseline != null) | {id, z_score: .baseline.z_score}'

# Compare two reports directly
usereport --output json > before.json
# ... reproduce the incident ...
usereport --output json > after.json
usereport diff before.json after.json
```

### Check tool availability

```sh
usereport check
```

```
+------------+-------------+------------------+--------+
| Category   | Name        | Binary           | Status |
+=====================================================+
| default    | mpstat      | mpstat           | ok     |
|------------+-------------+------------------+--------|
| default    | iostat      | iostat           | ok     |
|------------+-------------+------------------+--------|
| collectors | dmesg       | dmesg            | ok     |
|------------+-------------+------------------+--------|
| collectors | free        | free             | ok     |
|------------+-------------+------------------+--------|
| profiling  | perf        | perf             | ok     |
|------------+-------------+------------------+--------|
| profiling  | bpftrace    | bpftrace         | ok     |
|------------+-------------+------------------+--------|
| bpf        | runqlat     | runqlat-bpfcc    | ok     |
|------------+-------------+------------------+--------|
| bpf        | biolatency  | biolatency-bpfcc | ok     |
+------------+-------------+------------------+--------+
```

Covers every binary `usereport` might invoke — profile commands, direct collectors, profiling tools, eBPF tools. Exits 1 if anything is missing. Run it after install or when setting up a new host.

---

## Real-world scenarios

### Scenario A — TIME_WAIT exhaustion on a busy API gateway

**Symptom:** A Node.js reverse proxy starts refusing new connections under load. Application logs show `ECONNREFUSED` on outgoing calls. CPU and memory look fine.

```sh
usereport --output markdown
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
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.ip_local_port_range="1024 65535"

echo "net.ipv4.tcp_tw_reuse = 1" >> /etc/sysctl.conf
echo "net.ipv4.ip_local_port_range = 1024 65535" >> /etc/sysctl.conf
```

**Verify:**

```sh
usereport --output json > before.json
# ... apply sysctls ...
usereport --output json > after.json
usereport diff before.json after.json
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
usereport --output markdown
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

### [Warn] mem.swap_in_active
Pages are being swapped in from disk.

Evidence:
- `vmstat.swap_in` = 847
```

Two findings tell the full story: the OOM killer has already fired twice (`dmesg.oom_count = 2`), and the host is actively paging (`vmstat.swap_in = 847` pages in during the 1-second measurement window). The service is thrashing swap before the kernel kills it.

**Investigate:**

```sh
# Who did the OOM killer take out, and why?
dmesg -T | grep -i 'killed process'
# [Tue May  6 03:17:42 2025] Out of memory: Killed process 18423 (java) \
#   total-vm:12582912kB, anon-rss:7340032kB, file-rss:0kB

# What is the JVM heap configured to?
systemctl cat svc-worker | grep -i xmx
# ExecStart=/usr/bin/java -Xmx6g -jar /opt/svc-worker.jar

# What is the host's total RAM?
grep MemTotal /proc/meminfo
# MemTotal: 8388608 kB  →  8 GB total; -Xmx6g leaves only 2 GB for OS + JVM overhead

# Hand the full picture to an LLM
usereport --output llm --redact | your-ai-cli "what is killing this service?"
```

**Fix:**

The JVM heap ceiling (`-Xmx6g`) plus JVM overhead, OS, and other processes exceeds available RAM.

```sh
# Option 1: reduce heap to leave headroom
systemctl edit svc-worker   # change -Xmx6g → -Xmx5g
systemctl restart svc-worker

# Option 2: add swap as a buffer while arranging more RAM
fallocate -l 4G /swapfile && chmod 600 /swapfile
mkswap /swapfile && swapon /swapfile
```

**Record a baseline once stable:**

```sh
usereport baseline record --name stable
# Next day, after a deployment:
usereport --baseline stable --exit-on warn
# Exits 1 if mem.free_pct drops below 10% again
```

---

### Scenario C — CPU thermal throttling silently killing a batch job

**Symptom:** A nightly data-processing job that normally completes in 20 minutes is taking 90 minutes. No code was deployed. CPU utilization looks normal.

```sh
usereport --baseline last-tuesday --output markdown
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

### [Warn] Anomaly: cpu.freq_ratio     ← automatic baseline finding
Signal cpu.freq_ratio deviates from baseline (z_score = 28.5).
Baseline p50 = 0.98, observed = 0.41.
```

The paradox is now visible: `cpu.idle_pct` is around 60% — the CPU has capacity — yet the job takes 4.5× longer. The CPU is running at **41% of its rated frequency** because it's hot. The baseline comparison makes this unmissable: `cpu.freq_ratio` normally sits at 0.98 and is now 0.41, a z-score of 28.5.

**Investigate:**

```sh
# Temperature of all thermal zones
paste \
  <(ls /sys/class/thermal/thermal_zone*/type | xargs -I{} cat {}) \
  <(cat /sys/class/thermal/thermal_zone*/temp | awk '{printf "%.1f°C\n", $1/1000}')
# x86_pkg_temp    94.0°C   ← approaching TjMax (usually 95–105°C)

# Check fans and sensors via IPMI
ipmitool sdr type Fan
ipmitool sdr type Temperature
```

At 94 °C the processor is one degree below its thermal shutdown point. Clock speed is being reduced to shed heat — exactly what `cpu.freq_ratio` captures.

**Fix:**

```sh
cpupower frequency-set -g powersave   # temporary; buys time to diagnose hardware
# Common causes: clogged air filters, failed chassis fan, dried thermal paste
```

**Verify:**

```sh
usereport diff before-fix.json after-fix.json
```

```
Signals changed:
  cpu.freq_ratio     0.41  →  0.97   (+0.56)
  cpu.temp_celsius  94.0   →  61.0   (-33.0)

Findings only in before-fix.json:
  [Warn] cpu.frequency_throttling
  [Warn] Anomaly: cpu.freq_ratio (z_score 28.5)
```

The batch job completed in 22 minutes on the next run.

---

## Features

### Direct kernel signal collection

On Linux, `usereport` reads the kernel directly — no tool required, no parsing output that differs between distro versions:

| Signal | Source |
|--------|--------|
| `cpu.usr_pct`, `cpu.iowait_pct`, `vmstat.r` | `/proc/stat` |
| `disk.max_util_pct`, `disk.max_await_ms`, `disk.<dev>.*` | `/proc/diskstats` |
| `net.rx_drops`, `net.retrans_pct`, `net.tw_count`, `net.estab_resets` | `/proc/net/dev` + `/proc/net/snmp` + `/proc/net/sockstat` |
| `net.max_cpu_irq_pct` | `/proc/interrupts` |
| `cpu.freq_ratio`, `cpu.temp_celsius` | `/sys/devices/system/cpu/*/cpufreq/` + thermal zones |
| `cgroup.memory_bytes`, `cgroup.oom_kills`, `cgroup.pids_current` | cgroup v1 / v2, auto-detected |
| `mem.swap_in`, `mem.swap_out` | `/proc/vmstat` |
| `host.load_avg_1m`, `host.mem_total_bytes` | `/proc/loadavg`, `/proc/meminfo` |

On macOS, the equivalent signals are collected via native commands:

| Signal | Source |
|--------|--------|
| `host.load_avg_1m` | `sysctl vm.loadavg` |
| `cpu.usr_pct`, `cpu.sys_pct`, `cpu.idle_pct` | `iostat` |
| `net.rx_drops` | `netstat -i -b -n` |
| `net.retrans_pct` and TCP counters | `netstat -s -p tcp` |
| `mem.*` page stats | `vm_stat` |
| `swap.*` usage | `sysctl vm.swapusage` |

### Rule engine with a predicate DSL

Built-in rules fire findings when signals cross thresholds. Write your own in TOML and drop them in `~/.config/usereport/rules.d/`:

```toml
[[rule]]
id          = "net.retrans_high"
when        = "net.retrans_pct > 1"
severity    = "warn"
summary     = "TCP retransmit rate is elevated (> 1%)"
evidence    = ["net.retrans_pct"]
suggest     = ["ss -tin", "netstat -s | grep retransmit"]
description = "Sustained retransmits indicate congestion, packet loss, or a broken path."
links       = ["https://www.brendangregg.com/perf.html"]
```

Predicates support:

| Feature | Example |
|---------|---------|
| Simple threshold | `mem.free_pct < 10` |
| Cross-signal comparison | `vmstat.r > host.cpu_count` |
| Percentile stats | `cpu.usr_pct.p95 > 80` |
| Trend direction | `net.tw_count.trend == "rising"` |
| Boolean logic | `mem.free_pct < 5 AND mem.swap_in > 0` |

A broken rule file emits a `warn` finding and is skipped — it never breaks the run.

### Pattern correlator

Single-signal rules are fast. Multi-signal patterns catch the subtle failures:

| Pattern | Signals it correlates |
|---------|-----------------------|
| `time_wait_exhaustion` | `net.tw_count` + `net.connect_failures` |
| `lock_contention` | CPU saturation + high iowait + run-queue depth |
| `thundering_herd` | burst of short-lived processes + CPU spikes |
| `socket_leak` | rising `net.tw_count` without matching traffic |
| `nfs_stall` | iowait spike + NFS mount activity |
| `slab_leak` | rising kernel slab usage over time |

### Baselines and drift detection

Record a healthy snapshot. Every future run compares every signal against it automatically.

```sh
# Capture on a healthy Tuesday
usereport baseline record --name tuesday

# Next Friday at 3am when alerts fire:
usereport --baseline tuesday --output html -O incident.html
```

Signals that deviate more than 3 standard deviations get a `warn` finding. More than 6 standard deviations get `crit`. The finding shows you the baseline p50, the observed value, and the z-score — no guessing whether the deviation is meaningful.

### Workload-aware rules

Load a rule pack tuned for what's actually running:

```sh
usereport --workload postgres   # connection saturation, cache hit rate, lock waits
usereport --workload nginx      # connection count, error rate, accept queue depth
usereport --workload java       # GC pressure, heap saturation, thread count
usereport --workload kubelet    # pod count, evictions, image pull latency
```

### eBPF collectors (opt-in, Linux)

When you need to go deeper than `/proc`:

```sh
usereport --bpf   # runqlat, biolatency, tcpretrans, execsnoop, cachestat
```

Emits histogram signals with full percentile stats (`p50`, `p95`, `p99`). Falls back gracefully — if a BCC tool isn't installed, you get an `info` finding with the install hint instead of an error.

### CPU flamegraph, inline

```sh
usereport --profile-cpu 30s --output html -O report.html
```

Runs `perf record` for 30 seconds (bpftrace if perf isn't available), folds the stacks with [inferno](https://github.com/jonhoo/inferno), and embeds the SVG directly in the HTML report. No extra steps, no separate files.

<p align="center">
  <a href="docs/linux-cpu-flamegraph.jpg"><img src="https://raw.githubusercontent.com/lukaspustina/usereport-rs/master/docs/linux-cpu-flamegraph.jpg" width="96%" /></a>
</p>

### LLM-ready output

```sh
usereport --output llm --redact | your-ai-cli "diagnose this"
```

Produces a compact JSON document — signals, findings, checked-ok list, and raw command excerpts — structured for feeding to an LLM without token waste. `--redact` HMAC-hashes hostnames, IPs, and MACs so nothing sensitive leaves the machine.

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

Works for both rule IDs and raw signal IDs. Shows install hints when the source tool is missing.

---

## Platform support

| Capability | Linux | macOS |
|---|:---:|:---:|
| Direct kernel reads (`/proc`, `/sys`) | **yes** | no |
| cgroup v1/v2 signal collection | **yes** | no |
| eBPF collectors (`--bpf`) | **yes** | no |
| CPU flamegraph (`--profile-cpu`) | **yes** | no |
| Native sysctl / vm_stat collectors | no | **yes** |
| Rule engine, patterns, baselines | **yes** | **yes** |
| All output formats | **yes** | **yes** |
| Custom rules and signal extraction | **yes** | **yes** |

macOS support focuses on the signals available without root via native commands. The rule engine, baselines, workload packs, LLM output, and all other non-collection features work identically on both platforms.

---

## Installation

### Homebrew (macOS and Linux)

```sh
brew tap lukaspustina/usereport-rs https://github.com/lukaspustina/usereport-rs
brew install lukaspustina/usereport-rs/usereport-rs
```

The formula lives in the `Formula/` directory of the main repository (not a separate `homebrew-` repo), so Homebrew needs the explicit URL on the first tap. Updated automatically on every release.

### Debian / Ubuntu (amd64 and arm64)

`.deb` packages are attached to every [GitHub Release](https://github.com/lukaspustina/usereport-rs/releases). The filename includes the version, so the easiest way to grab the latest is:

```sh
# amd64
curl -s https://api.github.com/repos/lukaspustina/usereport-rs/releases/latest \
  | grep browser_download_url \
  | grep amd64.deb \
  | cut -d'"' -f4 \
  | xargs curl -LO
sudo dpkg -i usereport-rs_*_amd64.deb

# arm64
curl -s https://api.github.com/repos/lukaspustina/usereport-rs/releases/latest \
  | grep browser_download_url \
  | grep arm64.deb \
  | cut -d'"' -f4 \
  | xargs curl -LO
sudo dpkg -i usereport-rs_*_arm64.deb
```

Or pin a specific version directly:

```sh
VERSION=0.2.0
curl -LO https://github.com/lukaspustina/usereport-rs/releases/download/v${VERSION}/usereport-rs_${VERSION}_amd64.deb
sudo dpkg -i usereport-rs_${VERSION}_amd64.deb
```

### cargo binstall

```sh
cargo binstall usereport-rs
```

Fetches the pre-built binary for your platform without compiling. Requires [cargo-binstall](https://github.com/cargo-bins/cargo-binstall).

### From source

```sh
cargo install --all-features usereport-rs
```

Requires Rust 1.85+. Install via [rustup](https://rustup.rs) if needed.

### Optional dependencies (Ubuntu 24.04)

The binary runs without any dependencies — core signals come directly from `/proc` and `/sys`. Optional tools unlock additional signal sources:

```sh
# mpstat, pidstat, iostat, sar — CPU/disk/network profiling
sudo apt install sysstat

# perf — CPU flamegraph (--profile-cpu)
sudo apt install linux-tools-common linux-tools-$(uname -r)

# BCC tools — eBPF collectors (--bpf)
sudo apt install bpfcc-tools
```

Run `usereport check` after install to verify which tools are available on your system.

---

## Output formats

| Format | Flag | Best for |
|--------|------|----------|
| Markdown | `--output markdown` _(default)_ | Terminal reading, pasting into tickets |
| HTML | `--output html` | Sharing reports, embedded flamegraphs |
| JSON | `--output json` | Automation, dashboards, `jq` pipelines |
| LLM | `--output llm` | Feeding an AI for diagnosis |
| Custom | `--output template --output-template my.j2` | Your own Jinja2 template |

Write the output to a file with `-O <path>`:

```sh
usereport --output html -O /tmp/$(hostname)-$(date +%s).html
```

---

## Convert: re-render a saved report

`usereport convert` reads a JSON report produced by `--output json` and re-renders it in any format — without re-running a single command.

**When to use it:**

- You captured JSON during an incident but want HTML to share with your team now.
- You ran `usereport` on an air-gapped server and want to pipe the LLM output on your laptop.
- A cron job archives JSON snapshots nightly; you want to render them on demand.
- The server is already back to normal — but you still have the raw data.

```sh
# Capture the data once, on the server (or in a cron job)
usereport --output json -O /var/log/usereport/$(date +%Y%m%dT%H%M%S).json

# Later — render as HTML to share with your team
usereport convert /var/log/usereport/20260502T031500.json --output html -O report.html

# Or feed it to an LLM on a different machine
usereport convert incident.json --output llm --redact | your-ai-cli "diagnose this"

# Or pipe between tools (reads stdin when no file is given)
cat incident.json | usereport convert --output markdown

# Or re-render with your own template
usereport convert incident.json --output template --output-template custom.j2 -O custom.html
```

All formats are available: `markdown`, `html`, `json`, `llm`, `template`. The `--redact` flag only applies when `--output llm` — it HMAC-hashes hostnames, IPs, and MACs before output.

---

## Configuration

`usereport` ships with built-in configs for Linux and macOS. Override any of it with `--config`:

```sh
usereport --config /etc/usereport/custom.toml
```

### Profiles

Profiles select a focused subset of commands:

```sh
usereport --show-profiles        # list all profiles
usereport --profile mem          # virtual memory focus
usereport --profile net          # network focus
usereport --profile cpu          # CPU focus
usereport +mpstat -vmstat        # add/remove individual commands from the active profile
```

### Signal extraction from command output

Any command can emit signals by matching its stdout with a regex. Use a named `(?P<val>...)` capture group and pick an aggregation function:

```toml
[[command]]
name    = "my_app"
command = "journalctl -u my-app --since -1m"

# Count lines matching a pattern — no val group needed for count
[[command.extract]]
pattern   = 'ERROR'
signal_id = "my_app.error_count"
unit      = "count"
aggregate = "count"

# Extract a numeric value and take the last sample seen
[[command.extract]]
pattern   = 'latency_ms=(?P<val>\d+)'
signal_id = "my_app.latency_ms"
unit      = "ms"
aggregate = "last"   # count | last | min | max | avg
```

The emitted signal is immediately available to the rule engine on the same run:

```toml
[[rule]]
id       = "my_app.latency_spike"
when     = "my_app.latency_ms > 500"
severity = "warn"
summary  = "Application latency above 500ms"
```

### Command annotations

Two optional fields improve the `check` and `explain` output for any command:

```toml
[[command]]
name             = "sar_cpu"
command          = "sar -u 1 5"
install_hint     = "apt-get install sysstat"
what_to_look_for = "High %iowait means disk is the bottleneck. Idle near 0% means CPU is saturated."
```

- `install_hint` — shown in `usereport explain <command-name>` when the binary is missing
- `what_to_look_for` — surfaced in `usereport explain <command-name>`

### Custom rules

Drop TOML files in `~/.config/usereport/rules.d/`. They merge with the built-ins at startup. A broken file emits a `warn` finding and is skipped — it never breaks the run.

---

## Exit codes

```sh
usereport --exit-on warn   # exit 1 if any Warn or higher findings
usereport --exit-on crit   # exit 2 only for Crit findings
usereport --exit-on never  # always exit 0 (default)
```

Useful in CI, cron jobs, and post-deploy health checks:

```sh
# Alert if anything looks wrong after a deployment
usereport --exit-on warn || pagerduty-alert "post-deploy health check failed on $(hostname)"
```

---

## Baselines and drift

```sh
# Record a named baseline from the current run
usereport baseline record --name prod-$(date +%Y%m%d)

# List recorded baselines
usereport baseline list

# Compare any two JSON reports
usereport diff before.json after.json
```

Baselines are stored as rolling JSONL files (default window: 24 entries, configurable via `baseline_rolling_n` in `[defaults]`). Every successful run appends a snapshot unconditionally — no flag required.

---

## As a library

The core is published as a Rust library. Use it to embed signal collection and rule evaluation in your own tooling:

```toml
[dependencies]
usereport-rs = "0.2"
```

See [`examples/json_report.rs`](examples/json_report.rs) for a complete, up-to-date example using the current `Analysis` constructor.

The `TemplateRenderer` and `JsonRenderer` accept the report directly. All collector types, signal structs, and rule engine APIs are public.

---

## Contributing

Pull requests and issue reports are welcome.

```sh
make ci         # full pipeline: fmt-check, clippy, test, audit, deny, machete
make pre-push   # lighter: fmt-check + clippy + test
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
