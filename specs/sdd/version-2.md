# SDD: usereport v2 — From Data Collector to Diagnostic Tool

Status: Ready for Implementation
Original: /Users/lukas/Documents/src/usereport-rs/specs/sdd/version-2.md
Refined: 2026-05-01

## Overview

`usereport` v2 turns the existing data-collection tool into a diagnostic tool: it parses tool output and reads `/proc`/`/sys` directly, produces typed `Signal` values, evaluates declarative TOML rules to produce `Finding` records, and renders evidence-backed conclusions in Markdown, HTML, and JSON. The binary stays deterministic, offline, and single-static-binary; an LLM skill layer is strictly downstream, consuming the structured JSON output.

## Context & Constraints

- Rust 2018, MSRV 1.74. Lib crate + binary crate; `bin` feature gates `clap` v4, `comfy-table`, `env_logger`, `anyhow`, `human-panic`, `indicatif`.
- Existing public types that must not change signature: `Runner`, `Renderer<W>`, `Command`, `CommandResult`, `Analysis`, `AnalysisReport`, `Config`, `Defaults`, `Profile`, `Hostinfo`.
- `TemplateRenderer` uses minijinja v2 with `with_html_escape()` toggle and a custom `rfc2822` filter. HTML auto-escape is set via `set_auto_escape_callback`.
- Tests use `googletest` v0.14; new tests use `assert_that!` with googletest matchers.
- Always build and test with `--all-features`.
- Error handling: `thiserror` in lib crate; `anyhow` at CLI boundary (`src/cli/`).
- `Context::new()` is infallible (uses `rustix::system::uname()`).
- Per-OS bundled config selected at compile time via `#[cfg(target_os)]` in `src/cli/mod.rs::defaults`.

### New dependencies (additions to `Cargo.toml`)

| Crate | Version | Where | Notes |
|---|---|---|---|
| `humantime` | `2` | `[dependencies]` | Parses `--duration` / `--interval` strings (`5s`, `2m`, `1h30m`). |
| `sha2` | `0.10` | `[dependencies]` | HMAC-SHA-256 backing for `redact.rs` and HMAC keys. |
| `hmac` | `0.12` | `[dependencies]` | HMAC primitive used by `redact.rs`. |
| `which` | `6` | `[dependencies]` | `$PATH` resolution for `Command::exec()` and BPF tool detection. |
| `regex` | `1` | `[dependencies]` | Used by `dmesg.rs` event matchers and `redact.rs` value detection (IPs, MACs, etc.). |
| `inferno` | `0.11` | `[dependencies]` | Folds `perf script` output into a flamegraph SVG (Phase 9). |

`rustix` is already present; `rustix::fs::flock` is used for baseline JSONL locking (Req 15) — no additional crate needed. The `bpf` Cargo feature gates `src/collector/bpf.rs` (the BPF wrappers shell out to `bpftrace`/`bcc-tools` binaries; no Rust BPF crate dependency).

### `Cargo.toml` `include` updates

The current `include` lists `"contrib/*.j2"` and `"contrib/*.conf"`, which do not match files in subdirectories. Phase 0 must extend `include` to publish the new bundled assets:

```toml
include = [
  "README.md",
  "LICENSE",
  "contrib/*.j2",
  "contrib/*.conf",
  "contrib/rules/**/*.toml",
  "contrib/patterns/**/*.toml",
  "**/*.rs",
  "Cargo.toml",
]
```

`docs/schemas/` and `skills/` are intentionally not included in the published crate (they are repo-level assets, not library artefacts).

## Architecture

```
                       ┌─────────────────────────────────────────────────────┐
                       │                Existing v1 path (kept)              │
                       │ Command ──exec──> CommandResult ──► Renderer ──► out│
                       └─────────────────────────────────────────────────────┘
                                                │
                                                ▼
┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌─────────────────┐
│ Collector│ ► │  Parser  │ ► │  Signal  │ ► │   Rule   │ ► │     Finding     │
│  /proc,  │   │  vmstat, │   │ typed    │   │  engine  │   │ severity,       │
│  /sys,   │   │  iostat, │   │ metric + │   │  TOML    │   │ evidence chain, │
│  cmd out │   │  dmesg…  │   │ baseline │   │  rules   │   │ suggest cmds    │
└──────────┘   └──────────┘   └──────────┘   └──────────┘   └─────────────────┘
                                                                      │
                                                                      ▼
                                                          ┌───────────────────────┐
                                                          │ Renderer extensions   │
                                                          │ - Markdown SUMMARY +  │
                                                          │   FINDINGS sections   │
                                                          │ - HTML findings panel │
                                                          │ - JSON signals/find.  │
                                                          │ - --output llm        │
                                                          └───────────────────────┘
                                                                      │
                                                                      ▼
                                                          ┌───────────────────────┐
                                                          │ skills/usereport-     │
                                                          │ analyze/ (downstream) │
                                                          └───────────────────────┘
```

Key separations:
- A `Collector` implementation may read `/proc`/`/sys` directly or parse stdout of an existing `Command`. Both paths emit identical `Signal` types. On Linux, direct `/proc` read is preferred; parser-based is the fallback for macOS/BSD or absent paths.
- Rules are declarative TOML, shipped in `contrib/rules/`, loadable from `~/.config/usereport/rules.d/`. User rules supplement (do not replace) built-ins; a malformed user file emits a `warn` finding without poisoning built-ins.
- Patterns are multi-signal correlations, declared in `contrib/patterns/`; evaluated after the single-rule pass.

## Requirements

### Diagnostic core

1. The system shall provide a `Collector` trait with methods `id(&self) -> &str`, `collect(&self, ctx: &CollectCtx) -> Result<Vec<Signal>>`, and `supports_sampling(&self) -> bool` (default `false`).
2. The system shall define a `Signal` type with: stable `id: String` (e.g. `cpu.iowait_pct`), `value: SignalValue`, `unit: Unit`, `at: chrono::DateTime<Local>`, `samples: Option<Vec<f64>>` (populated when `--duration` is used), and `baseline: Option<BaselineStats>` (`None` unless `--baseline NAME` is passed or the rolling baseline covers the signal ID).
3. The system shall define a `Rule` type loaded from TOML with fields: `id: String`, `when: Predicate` (parsed from a string expression — see Data Models for grammar), `severity: Severity`, `summary: String`, `evidence_ids: Vec<String>`, `suggest: Vec<String>`.
4. The system shall ship at minimum 15 built-in rules in `contrib/rules/*.toml` covering: CPU saturation, run-queue saturation, iowait elevation (threshold: iowait_pct > 20%), memory pressure (free < 10% of total), swap activity (swap-in rate > 0), OOM in dmesg, disk `%util` saturation (> 90%), disk await elevation (await > 100 ms), network retransmit rate (retrans_pct > 1%), network drops (rx_drops > 0), TIME_WAIT count (tw_count > 28 000), CPU frequency throttling (freq_ratio < 0.8), IRQ imbalance (max_cpu_irq_pct > 80%), blocked tasks in dmesg (> 0), and EXT4/XFS errors in dmesg (> 0). Thresholds stated here are the initial defaults; all thresholds live in TOML and are user-overridable.
5. The system shall produce a `Finding` for each rule whose `when` predicate matches; a `Finding` carries `id`, `severity`, `summary`, the actual `Evidence` values (signal ID + observed value) that triggered it, and `suggest`.
6. The system shall render a `FINDINGS` section above the existing raw output in Markdown and HTML templates, ordered by severity (`Crit` → `Warn` → `Info`); within a severity, findings are ordered lexicographically by `Finding::id`. A `SUMMARY` block (host, kernel, cores, mem, load, top concern) precedes the findings section.
7. The system shall extend the JSON renderer to include top-level arrays `signals`, `findings`, and `checked_ok` alongside the existing `command_results` in the `AnalysisReport` serialization.
8. The system shall return exit code 0 when `--exit-on=never` (the default) regardless of findings. When `--exit-on=warn`, exit code is 1 if any `warn` finding fires and 0 otherwise. When `--exit-on=crit`, exit code is 2 if any `crit` finding fires and 0 otherwise. Exit codes 1 and 2 are never emitted when `--exit-on=never`.

### Direct collectors and delta engine

9. The system shall provide a delta engine that reads monotonic counters from `/proc/stat`, `/proc/diskstats`, `/proc/net/dev`, and `/proc/net/snmp` at two time points separated by at least 1 second (sleeping to reach 1 s if the elapsed time is shorter), then emits per-second rates as signals.
10. The system shall include a `cgroup` collector that detects cgroup v1 vs v2 and reads `cpu.stat`, `memory.current`, `memory.max`, `memory.events`, `io.stat`, and `pids.current` when run inside a cgroup or when pointed at one via `--cgroup <path>`.
11. The system shall include a `cpufreq` collector reading `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` and `/sys/class/thermal/thermal_zone*/temp`, emitting frequency-vs-max ratio (`freq_ratio`) and thermal-throttle signals.
12. The system shall include an `interrupts` collector reading `/proc/interrupts` and emitting per-CPU IRQ distribution signals (notably NIC interrupt concentration, expressed as `max_cpu_irq_pct`).

### Baseline and diff

13. The system shall persist baselines as JSON under `${XDG_DATA_HOME:-~/.local/share}/usereport/baselines/<name>.json` via a `usereport baseline record [--name NAME]` subcommand. If the directory is read-only or cannot be created, the command shall print an error to stderr and exit 1.
14. The system shall accept `--baseline NAME` on a normal run; signals shall be annotated with `baseline_p50`, `baseline_p95`, `mad`, and `z_score`; rules may reference `z_score` in their `when` predicate.
15. The system shall maintain a rolling baseline in `${XDG_DATA_HOME}/usereport/baselines/_rolling.jsonl`. Every successful run appends one JSONL record; when the file contains more than `baseline_rolling_n` (configurable, default 24) records the oldest records are pruned on write. Concurrent writes are serialised via `rustix::fs::flock` (no additional crate). Malformed JSONL lines are skipped (logged at `debug`) without aborting. Signals whose `|z_score| > 3` (computed from the rolling baseline) produce automatic `warn` findings; `|z_score| > 6` produces `crit` findings.
16. The system shall provide `usereport diff <a.json> <b.json>` printing per-signal deltas (signal ID, value-in-a, value-in-b, delta), plus three sections: findings present only in `a`, findings present only in `b`, and findings whose severity changed between runs. Default output is plain text; `--output json` emits a structured JSON object reusing the existing `OutputType` enum.

### Time-sampled collection

17. The system shall accept `--duration <DURATION> --interval <DURATION>`. The number of samples collected per signal is `N = floor(duration / interval) + 1`. `--interval` defaults to `5s` when `--duration` is present. `--interval` without `--duration` is a CLI error. `--duration` and `--repetitions` are mutually exclusive; using both is a CLI error with a message directing the user to `--duration`.
18. For sampled signals the system shall record per-sample values in `Signal::samples` and compute summary statistics `min`, `max`, `p50`, `p95`, and `trend` (see Data Models). Rules may reference these via dotted suffix syntax (e.g. `cpu.iowait_pct.p95 > 30`). When a rule references a bare signal ID and the signal has samples, the predicate uses the `p50` of samples.

### dmesg miner and pattern catalog

19. The system shall include a structured `dmesg` parser (`src/collector/dmesg.rs`) detecting: OOM kills (victim PID + comm), segfaults, blocked-task warnings (`task X blocked for more than 120 seconds`), machine check exceptions (MCEs), EXT4/XFS errors, NIC link flaps, and `blk_update_request` I/O errors. Each detected event becomes a `Signal` with a boolean or count value.
20. The system shall ship a pattern catalog in `contrib/patterns/` with at minimum six patterns: lock contention, NFS/network FS stall, TIME_WAIT exhaustion, kernel slab leak, thundering herd, and application socket leak. Each pattern is a TOML file declaring the multi-signal predicate and `suggest` commands. The pattern correlator runs after the single-rule pass and produces `Finding` records with `kind = "pattern"` to distinguish them from single-rule findings.

### LLM-friendly output and skill

21. The system shall provide `--output llm` emitting a schema-versioned JSON document. The document must contain: `schema_version: "1"`, `host` summary, `signals` (with baseline and z-score where available), `findings` (with evidence-ID chains), `checked_ok` (signal IDs investigated and clean), and `raw_excerpts` (dmesg lines that triggered findings). The JSON Schema lives at `docs/schemas/llm-output-v1.json`.
22. The system shall accept `--redact` (only meaningful with `--output llm`). Redaction uses SHA-256 HMAC keyed on `USEREPORT_REDACT_SALT` (env var). When `USEREPORT_REDACT_SALT` is unset, a fixed compile-time constant is used (documented as providing weak privacy; same host always hashes identically but hashes are not secret). Redacted fields: hostnames, IPv4 addresses, IPv6 addresses, MAC addresses, usernames, and process command-line arguments. Values not matching any pattern pass through unchanged.
23. The repository shall contain a Claude Code skill at `skills/usereport-analyze/SKILL.md`. The skill reads an `--output llm` document from stdin or from a positional file-path argument. It produces: a one-sentence TL;DR, a ranked likely root-cause with evidence chain citing finding IDs, alternative hypotheses with disambiguation criteria, ordered next-step commands, and an explicit "ruled out" list. The skill must include a constraint: never fabricate metric values; all claims must cite a finding ID or signal ID from the input document. The skill must refuse analysis when `schema_version` is not `"1"`, producing an explicit version-mismatch error.
24. The repository shall contain `skills/usereport-analyze/fixtures/` with at least five paired `(input.json, reference.md)` examples: `good-box/`, `memory-pressure/`, `io-bound/`, `thermal-throttle/`, `time-wait-exhaustion/`. Each `input.json` must be schema-valid against `docs/schemas/llm-output-v1.json`.

### CLI cleanup and modernization (Phase 0)

25. The system shall add `-O, --output-file <PATH>` writing rendered output to a file (parent directories created automatically); when absent, output goes to stdout (current behaviour).
26. The system shall rename `OutputType::Hbs` to `OutputType::Template`. The string alias `"hbs"` shall remain accepted by the clap parser and shall print an unconditional `eprintln!` deprecation warning (not gated on `RUST_LOG`). The `--output-template` flag and validation logic are updated to reference `OutputType::Template`.
27. The system shall resolve external commands via `$PATH` in bundled configs (no hardcoded `/usr/bin/...` paths). When a command binary is absent from `$PATH` at execution time, the runner shall produce no `CommandResult::Error`; instead it shall record a `CommandResult::SkippedMissing` and the report shall emit an `info`-severity finding naming the missing binary. The command's section is omitted from Markdown/HTML output entirely (no empty section header).
28. The HTML template (`contrib/html.j2`) shall not load any third-party CSS or JS over the network; all required styles shall be inlined in the template.
29. The bundled macOS configuration (`contrib/osx.conf`) shall include `mem` and `net` profiles with coverage parity to Linux, and shall not contain placeholder `echo` commands. Every command in `default`, `mem`, and `net` profiles must produce `CommandResult::Ok` or `CommandResult::SkippedMissing` on a clean macOS system, never `CommandResult::Error`.
30. The bundled Linux configuration (`contrib/linux.conf`) shall use `ss` in place of `netstat` and shall declare optional sysstat-based commands as skip-on-missing.

### Optional / opt-in capabilities

31. The system shall provide `--bpf` (off by default) which enables BPF-based collectors (`runqlat`, `biolatency`, `tcpretrans`, `execsnoop`, `cachestat`). Each collector detects its tool independently at collect time using `which::which("<tool>")` and emits an `info` finding when the tool is absent. The `bpf` feature is a separate Cargo feature (off by default; off in `cargo install`).
32. The system shall accept `--workload <NAME|none>`. When `--workload <NAME>` is given, the matching rule pack from `contrib/rules/workloads/<NAME>.toml` is loaded in addition to base rules. When `--workload none` (the default), no workload pack is loaded. Auto-detection of workloads is not run by default; `--workload` requires an explicit name.
33. The system shall provide `--profile-cpu <DURATION>` which runs `perf record -F 99 -ag` (or `bpftrace profile:hz:99` when `--bpf`) for the specified duration, folds the output into a flamegraph SVG, and embeds it inline in the HTML report. When neither `perf` nor `bpftrace` is available, an `info` finding is emitted and the flamegraph section is omitted.
34. The system shall provide `usereport explain <id>` printing the metric definition, what raises it, what to investigate, and links — sourced from `description` and `links` annotations on built-in rules and signals. Missing topics print a friendly listing of known IDs.

## File & Module Structure

New files (additions). Existing v1 files are modified only where noted.

```
src/
  collector/
    mod.rs            — Collector trait, CollectCtx, CollectorResult, registry
    cpu.rs            — /proc/stat delta engine + mpstat parser
    memory.rs         — /proc/meminfo + free parser + cgroup memory.*
    disk.rs           — /proc/diskstats delta + iostat parser
    network.rs        — /proc/net/dev delta + ss/sar parsers
    interrupts.rs     — /proc/interrupts parser; emits max_cpu_irq_pct
    cpufreq.rs        — scaling_cur_freq + thermal_zone; emits freq_ratio
    cgroup.rs         — v1/v2 detection + readers
    dmesg.rs          — structured dmesg event parser
    bpf.rs            — Cargo feature "bpf": bpftrace/bcc wrappers
  signal.rs           — Signal, SignalValue, Unit, BaselineStats, SampleStats
  finding.rs          — Finding, Severity, Evidence, FindingKind
  rule/
    mod.rs            — Rule, RuleEngine, predicate evaluator, TOML loader
    builtin.rs        — include_str! of contrib/rules/*.toml
  pattern/
    mod.rs            — Pattern, catalog loader, multi-signal correlator
  baseline/
    mod.rs            — public API: record(), load(), annotate()
    store.rs          — XDG path resolution, JSON persistence, flock locking
    stats.rs          — median(), mad(), z_score(), SampleStats helpers
  workload/
    mod.rs            — rule-pack loader; process-list detection removed (YAGNI)
  llm.rs              — LlmOutput struct (schema_version = "1") + serde;
                        lib-core (no feature gate) so library consumers can
                        produce LLM output too. Pulls in `sha2`, `hmac`.
  redact.rs           — SHA-256 HMAC redaction; USEREPORT_REDACT_SALT;
                        lib-core (no feature gate); shares `sha2` + `hmac` deps
                        with llm.rs
  cli/
    mod.rs            — MODIFIED: extended Opt (new flags), OutputType::Template,
                        OutputType::Llm, -O/--output-file, --exit-on, --baseline,
                        --duration, --interval, --cgroup, --bpf, --workload,
                        --redact, --profile-cpu
    config.rs         — unchanged
    explain.rs        — `usereport explain <id>` subcommand
    diff.rs           — `usereport diff <a.json> <b.json>` subcommand
    baseline_cmd.rs   — `usereport baseline {record,list,show,delete}`
  command.rs          — MODIFIED: add CommandResult::SkippedMissing variant;
                        exec() checks PATH before fork
  analysis.rs         — MODIFIED: AnalysisReport gains signals, findings,
                        checked_ok fields; Analysis::run() invokes collectors
                        and rule engine after command execution
  runner.rs           — unchanged
  renderer.rs         — unchanged (Renderer<W> trait stable)
contrib/
  rules/
    cpu.toml
    memory.toml
    disk.toml
    network.toml
    dmesg.toml
    workloads/
      postgres.toml
      java.toml
      nginx.toml
      kubelet.toml
  patterns/
    lock_contention.toml
    nfs_stall.toml
    time_wait.toml
    slab_leak.toml
    thundering_herd.toml
    socket_leak.toml
  osx.conf            — MODIFIED: mem/net profiles, no echo placeholders, $PATH cmds
  linux.conf          — MODIFIED: ss instead of netstat, $PATH-resolved, skip-missing
  html.j2             — MODIFIED: inline CSS, SUMMARY + FINDINGS sections
  markdown.j2         — MODIFIED: SUMMARY + FINDINGS sections
docs/
  schemas/
    llm-output-v1.json
skills/
  usereport-analyze/
    SKILL.md
    fixtures/
      good-box/{input.json,reference.md}
      memory-pressure/{input.json,reference.md}
      io-bound/{input.json,reference.md}
      thermal-throttle/{input.json,reference.md}
      time-wait-exhaustion/{input.json,reference.md}
.github/workflows/
  ci.yml              — replaces any Azure Pipelines references
```

## Data Models

```rust
// src/signal.rs

pub struct Signal {
    pub id:       String,                 // stable dotted ID, e.g. "cpu.iowait_pct"
    pub value:    SignalValue,
    pub unit:     Unit,
    pub at:       chrono::DateTime<chrono::Local>,
    pub samples:  Option<Vec<f64>>,       // present when --duration is used
    pub baseline: Option<BaselineStats>,  // present when --baseline NAME is passed
                                          // or rolling baseline covers this signal ID
}

pub enum SignalValue {
    F64(f64),
    I64(i64),
    Bool(bool),
    Text(String),
}

pub struct BaselineStats {
    pub p50:     f64,
    pub p95:     f64,
    pub mad:     f64,
    pub z_score: f64,
}

/// Summary statistics computed from Signal::samples.
/// Stored separately (not in Signal) so the predicate evaluator can access
/// them via the ".p50", ".p95", ".trend" suffix on signal IDs.
pub struct SampleStats {
    pub min:   f64,
    pub max:   f64,
    pub p50:   f64,
    pub p95:   f64,
    pub trend: Trend,
}

/// Trend is flat when |slope| < 5% of p50 per interval; rising or falling otherwise.
/// "slope" is the linear regression coefficient over per-sample values.
pub enum Trend { Rising, Falling, Flat }

pub enum Unit {
    Pct,            // percentage 0–100
    MillisPerOp,    // ms/op (disk await)
    BytesPerSec,    // bytes/s
    Count,          // dimensionless integer count
    Iops,           // I/O operations per second
    Microseconds,   // µs
    Hz,             // CPU frequency
    Celsius,        // thermal sensor
    None,           // no unit
}


// src/finding.rs

pub struct Finding {
    pub id:       String,          // rule or pattern id that fired
    pub kind:     FindingKind,     // Rule | Pattern
    pub severity: Severity,
    pub summary:  String,
    pub evidence: Vec<Evidence>,   // signal_id + observed value at fire time
    pub suggest:  Vec<String>,     // ordered next-step commands
}

pub enum FindingKind { Rule, Pattern }

pub enum Severity { Info, Warn, Crit }

pub struct Evidence {
    pub signal_id: String,
    pub observed:  SignalValue,
}


// src/rule/mod.rs

pub struct Rule {
    pub id:           String,
    pub when:         Predicate,    // parsed from TOML `when` string
    pub severity:     Severity,
    pub summary:      String,
    pub evidence_ids: Vec<String>,
    pub suggest:      Vec<String>,
}

/// Predicate DSL grammar (parsed via a hand-written recursive-descent parser
/// or a small PEG; no external parser-generator required given the small grammar).
///
/// expr   ::= term (("AND" | "OR") term)*
/// term   ::= path op value
/// path   ::= IDENT ("." IDENT)*            // e.g. cpu.iowait_pct or cpu.iowait_pct.p95
/// op     ::= ">" | "<" | ">=" | "<=" | "==" | "!="
/// value  ::= NUMBER | BOOL | STRING
///
/// Path resolution:
///   - A bare signal path (no suffix) resolves to the signal's `value` field.
///     When the signal has samples, bare path resolves to SampleStats::p50.
///   - A path ending in ".p50", ".p95", ".p99", ".min", ".max" resolves to the
///     corresponding SampleStats field (error if signal has no samples).
///   - A path ending in ".trend" resolves to SampleStats::trend compared to
///     the string literals "rising", "falling", "flat".
///   - A path starting with "host." resolves to CollectCtx host properties
///     (e.g. "host.cpu_count").
///   - A path starting with "z_score" on a signal resolves to
///     Signal::baseline.z_score (None treated as 0.0).


// src/collector/mod.rs

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read {path}: {source}")]
    ReadFailed { path: std::path::PathBuf, source: std::io::Error },
    #[error("failed to parse output of '{collector}': {reason}")]
    ParseFailed { collector: String, reason: String },
    #[error("collector '{collector}' is unavailable on this host: {reason}")]
    Unavailable { collector: String, reason: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct CollectCtx {
    pub duration:    Option<std::time::Duration>,
    pub interval:    Option<std::time::Duration>,
    pub cgroup_path: Option<std::path::PathBuf>,
    pub baseline:    Option<std::sync::Arc<crate::baseline::BaselineStore>>,
    pub cpu_count:   usize,   // for "host.cpu_count" in predicates
}

pub trait Collector: std::fmt::Debug + Send + Sync {
    fn id(&self) -> &str;
    fn collect(&self, ctx: &CollectCtx) -> collector::Result<Vec<Signal>>;
    fn supports_sampling(&self) -> bool { false }
}
```

This follows the per-module `thiserror`-derived `Error` + `pub type Result<T, E = Error>` pattern already used in `analysis.rs`, `runner.rs`, `renderer.rs`, and `cli/config.rs`. The same per-module pattern applies to new modules `rule::Error`, `baseline::Error`, `pattern::Error`, `workload::Error`, `redact::Error`, `cli::diff::Error`, `cli::explain::Error`, `cli::baseline_cmd::Error`. There is no top-level `crate::Result`.

## API Contracts

### `AnalysisReport` extensions (src/analysis.rs)

`AnalysisReport` gains three new fields alongside the existing `command_results`. Field visibility follows the existing pattern: fields are `pub(crate)`, exposed via accessor methods.

```rust
pub(crate) signals:    Vec<Signal>,
pub(crate) findings:   Vec<Finding>,
pub(crate) checked_ok: Vec<String>,   // signal IDs evaluated and found normal

impl AnalysisReport {
    pub fn signals(&self)    -> &[Signal]  { &self.signals }
    pub fn findings(&self)   -> &[Finding] { &self.findings }
    pub fn checked_ok(&self) -> &[String]  { &self.checked_ok }
}
```

`AnalysisReport::new()` is extended with three additional parameters (positional, after the existing ones) so external constructions remain explicit. `JsonRenderer` serialises the three new fields as top-level arrays via `#[derive(Serialize)]` on `AnalysisReport` (the existing derive remains; field-name renames via `#[serde(rename = "...")]` are not required since field names already match desired JSON keys). `TemplateRenderer` exposes them to minijinja templates as `signals`, `findings`, and `checked_ok`.

### `CommandResult` extension (src/command.rs)

The existing variants `Success { command, run_time_ms, stdout }`, `Failed { command, run_time_ms, stdout }`, `Timeout { command, run_time_ms }`, and `Error { command, reason }` are preserved unchanged. One variant is added:

```rust
pub enum CommandResult {
    // existing variants unchanged: Success, Failed, Timeout, Error
    SkippedMissing { command: Command, binary: String },   // binary not found on $PATH
}
```

`SkippedMissing` carries the `Command` for symmetry with the other variants. When `Command::exec()` detects the command binary is absent from `$PATH` (resolved via `which::which`), it returns `CommandResult::SkippedMissing` instead of forking. The CLI reports this as an `info` finding; the section is omitted from Markdown/HTML output.

### `LlmOutput` (src/llm.rs)

```rust
#[derive(serde::Serialize)]
pub struct LlmOutput {
    pub schema_version: &'static str,   // always "1"
    pub host:           LlmHost,
    pub signals:        Vec<Signal>,
    pub findings:       Vec<Finding>,
    pub checked_ok:     Vec<String>,
    pub raw_excerpts:   Vec<RawExcerpt>,
}

pub struct LlmHost {
    pub hostname:        String,   // from existing Context::hostname()
    pub kernel:          String,   // from existing Context::uname()
    pub cpu_count:       usize,    // from signal "host.cpu_count"
    pub mem_total_bytes: u64,      // from signal "host.mem_total_bytes"
    pub load_avg_1m:     f64,      // from signal "host.load_avg_1m"
}

pub struct RawExcerpt {
    pub source:    String,   // e.g. "dmesg"
    pub finding_id: String,  // finding that triggered inclusion
    pub lines:     Vec<String>,
}
```

### Predicate evaluator (src/rule/mod.rs)

```rust
pub fn evaluate(predicate: &Predicate, signals: &HashMap<String, &Signal>, ctx: &CollectCtx)
    -> Result<bool, PredicateError>;
```

Returns `Ok(false)` — never an error — when a referenced signal ID is absent (treating absence as "not triggered"). Returns `Err(PredicateError)` only on type mismatch (e.g. comparing a `Text` signal with a number).

## Configuration

Existing TOML config (`Defaults`, `Profile`, `Hostinfo`, `Command`) is preserved without breaking changes. New optional fields:

```toml
[defaults]
exit_on              = "never"   # never | warn | crit; default "never"
baseline_rolling_n   = 24        # rolling-baseline window size

# Inline rule (also loadable from contrib/rules/*.toml)
[[rule]]
id       = "cpu.runqueue_saturation"
when     = "vmstat.r > host.cpu_count"
severity = "warn"
summary  = "Run queue exceeds core count — CPU saturated"
evidence = ["vmstat.r", "host.cpu_count"]
suggest  = ["pidstat 1 5", "perf top -F 99"]

# Inline pattern (also loadable from contrib/patterns/*.toml)
[[pattern]]
id       = "time_wait_exhaustion"
when     = "net.tw_count > 28000 AND net.connect_failures > 0"
severity = "crit"
summary  = "TIME_WAIT exhaustion likely"
suggest  = ["sysctl net.ipv4.tcp_tw_reuse", "sysctl net.ipv4.ip_local_port_range"]
```

### CLI flags (additions only; existing flags unchanged)

| Flag | Type | Default | Notes |
|---|---|---|---|
| `-O, --output-file` | path | (stdout) | creates parent directories |
| `--exit-on` | `never\|warn\|crit` | `never` | exit code semantics (Req 8) |
| `--baseline` | NAME | (none) | annotate signals; enable outlier rules |
| `--duration` | duration string | (none) | enables sampled mode |
| `--interval` | duration string | `5s` | only valid with `--duration` |
| `--cgroup` | path | (none) | target cgroup path for cgroup collector |
| `--bpf` | flag | false | enable BPF collectors (requires `bpf` feature) |
| `--workload` | NAME | `none` | load rule pack from `contrib/rules/workloads/<NAME>.toml` |
| `--redact` | flag | false | apply redaction to `--output llm` output |
| `--profile-cpu` | duration string | (none) | generate flamegraph SVG in HTML output |

New `OutputType` variants:

```rust
pub enum OutputType {
    Template,   // renamed from Hbs; "hbs" still accepted as deprecated alias
    Html,
    Json,
    Markdown,
    Llm,        // new: --output llm
}
```

### Environment variables

| Variable | Effect |
|---|---|
| `RUST_LOG` | Controls log verbosity (existing) |
| `USEREPORT_RULES_DIR` | Override built-in rules directory |
| `USEREPORT_BASELINE_DIR` | Override XDG baseline directory |
| `XDG_DATA_HOME` | Respected for baseline storage path |
| `USEREPORT_REDACT_SALT` | HMAC key for `--redact`; weak-privacy fallback when unset |

## Error Handling

| Failure | Trigger | Behaviour | User-visible |
|---|---|---|---|
| Collector binary missing from `$PATH` | e.g. `mpstat`, `ss`, `bpftrace` absent | `CommandResult::SkippedMissing`; section omitted from output; `info` finding naming binary | Yes — finding in FINDINGS section |
| `/proc` or `/sys` path absent | `ENOENT` on direct read (container, BSD) | Collector silently skipped; signal ID recorded in `checked_ok` with value absent | Only via `RUST_LOG=debug` |
| Rule TOML parse error | Malformed file in `~/.config/usereport/rules.d/` | That file skipped; built-ins still run; `warn` finding with path + parse error | Yes — finding in FINDINGS section |
| Baseline directory read-only | Cannot create XDG baseline dir | `usereport baseline record` prints error to stderr, exits 1 | Yes — stderr + exit 1 |
| Baseline file corrupted | JSON parse failure on load | Treated as missing baseline; file not deleted | Yes — warning to stderr |
| Rolling JSONL partial-line corruption | Mid-stream truncation | Malformed lines skipped; logged at `debug`; valid entries processed | No (debug only) |
| BPF tool present but no privilege | `bpftrace` returns EPERM | `info` finding "BPF needs CAP_BPF or root" | Yes — finding in FINDINGS section |
| `--profile-cpu` without `perf`/`bpftrace` | Neither binary on `$PATH` | `info` finding; flamegraph section omitted from HTML | Yes — finding in FINDINGS section |
| LLM output schema version mismatch | Skill receives `schema_version != "1"` | Skill refuses analysis with explicit version-mismatch message | Yes — skill output |
| `--redact` value not matched by any pattern | Pattern not recognised | Value passes through unchanged | No (by design) |
| `--interval` given without `--duration` | CLI parse time | CLI error with message; exits 1 | Yes — clap error message |
| `--duration` and `--repetitions` both given | CLI parse time | CLI error; exits 1 | Yes — clap error message |
| Delta engine window < 1 s | `/proc` reads faster than 1 s | Sleep until 1 s elapsed before computing rates | No |

## Implementation Phases

Each phase is independently committable and testable. Phases 0–4 are the v2 critical path; 5–8 are independently shippable add-ons.

### Phase 0 — v1.5 cleanup (prerequisites)

Tasks:
- Update `contrib/linux.conf` and `contrib/osx.conf`: replace hardcoded `/usr/bin/...` paths with bare binary names resolved via `$PATH`; replace `netstat` with `ss` in Linux config; add `mem` and `net` profiles to macOS config; remove echo placeholder commands.
- Add `CommandResult::SkippedMissing` variant to `src/command.rs`; implement PATH-check logic in `exec()`.
- Add `-O, --output-file` to `Opt` in `src/cli/mod.rs`; route output to file or stdout.
- Rename `OutputType::Hbs` to `OutputType::Template`.
- Update `impl FromStr for OutputType` in `src/cli/mod.rs` to map `"hbs"` → `OutputType::Template` and emit `eprintln!("warning: --output hbs is deprecated; use --output template")` (unconditional, not gated on `RUST_LOG`). Keep `"template"` as the canonical spelling.
- Update clap `value_enum` to expose `template`, `html`, `json`, `markdown`, `llm`; the `hbs` alias is handled by the `FromStr` impl.
- Update `src/cli/mod.rs` validation (`Opt::validate`) for `OutputType::Template` (replacing the existing `OutputType::Hbs` arm).
- Inline CSS into `contrib/html.j2`; remove Bootstrap CDN link.
- Add `contrib/rules/`, `contrib/patterns/` directory skeletons (empty placeholder files acceptable in this phase).
- Add `.github/workflows/ci.yml` with the following jobs, running on a matrix of `ubuntu-latest` and `macos-latest`, using `dtolnay/rust-toolchain@stable`:
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-features`
  - `cargo audit --deny-warnings` (Linux only is acceptable; install via `cargo install cargo-audit --locked`)
- Remove the Azure Pipelines badge from `README.md` (line 3 of the current README); archive any pre-existing `.ci/azure-pipelines.yml` to `.ci/azure-pipelines.yml.archived` (or delete if empty).
- Add `Cargo.toml` `include` entries for `contrib/rules/**/*.toml` and `contrib/patterns/**/*.toml` (see Context & Constraints — `Cargo.toml include` updates).
- Update `README.md`: drop Bionic/Azure mentions; add `cargo binstall`; mention Jinja2.

Completion criteria:
- `cargo test --all-features` green on Linux and macOS in CI.
- `usereport --help` shows `--output template` (not `hbs`).
- `usereport --output hbs --output-template f.j2` prints deprecation warning to stderr.
- HTML report renders fully offline (no network requests).
- Every command in macOS `default`/`mem`/`net` profiles produces `CommandResult::Ok` or `CommandResult::SkippedMissing`, never `CommandResult::Error`.
- Tag `v1.5.0`.

### Phase 1 — Diagnostic foundation

Tasks:
- Add `src/signal.rs`, `src/finding.rs` with all types defined in Data Models.
- Add `src/collector/mod.rs` with `Collector` trait and `CollectCtx`.
- Implement first three parser-based collectors: `cpu.rs` (parses `vmstat`/`mpstat` stdout), `memory.rs` (parses `free` stdout), `disk.rs` (parses `iostat` stdout). No `/proc` direct reads yet.
- Add `src/rule/mod.rs`: TOML loader, predicate parser (recursive descent over DSL grammar), `RuleEngine::run()`.
- Add `src/rule/builtin.rs`: `include_str!` the 15 built-in rule TOML files from `contrib/rules/`.
- Extend `AnalysisReport` with `signals`, `findings`, `checked_ok` fields; wire collectors and rule engine into `Analysis::run()`.
- Update `JsonRenderer` to serialise the three new fields.
- Update `contrib/markdown.j2` and `contrib/html.j2` with `SUMMARY` and `FINDINGS` blocks.
- Add `--exit-on=never|warn|crit` to `Opt`; implement exit-code logic in `src/cli/mod.rs::main()`.
- Write fixture-based tests using the existing inline `#[cfg(test)] mod tests` pattern with `googletest` `assert_that!` macros — no `insta`, no external snapshot crate. Fixtures live under `tests/fixtures/` and are loaded with `include_str!` (compile-time) or `std::fs::read_to_string` (runtime):
  - `tests/fixtures/collectors/<collector>/<variant>.txt` — captured tool stdout (at least two distro variants per parser-based collector).
  - `tests/fixtures/collectors/<collector>/<variant>.expected.json` — expected serialised `Vec<Signal>`.
  - `tests/fixtures/rules/<rule_id>/match.signals.json` and `nomatch.signals.json` — input signal sets producing expected matching / non-matching outcomes for each built-in rule.
  - Tests compare actual `Vec<Signal>`/`Vec<Finding>` against the expected JSON via `assert_that!(actual, eq(&expected))` after deserialising both.

Completion criteria:
- Rule-engine fixture suite passes (deterministic: same input → same output, byte-identical).
- Fixture suite covers at least one normal and one degenerate output per parser-based collector.
- `--output json` on a fixed synthetic signal set produces valid JSON with `signals`, `findings`, `checked_ok` arrays.
- Markdown report has SUMMARY and FINDINGS sections at top.
- `--exit-on=warn` with one warn finding exits 1; `--exit-on=never` exits 0.

### Phase 2 — Baseline & diff

Tasks:
- Implement `src/baseline/stats.rs`: `median()`, `mad()`, `z_score()` functions; unit tests with known inputs.
- Implement `src/baseline/store.rs`: XDG path resolution, JSON read/write, `flock`-based locking, rolling JSONL pruning.
- Implement `src/baseline/mod.rs` public API: `record()`, `load()`, `annotate()`.
- Add `src/cli/baseline_cmd.rs`: `usereport baseline record [--name NAME]`, `list`, `show <name>`, `delete <name>`.
- Add `--baseline NAME` to `Opt`; call `annotate()` on signals before rule evaluation.
- Implement auto-outlier finding generation from rolling baseline (`|z| > 3` → `warn`; `|z| > 6` → `crit`).
- Add `src/cli/diff.rs`: `usereport diff <a.json> <b.json>`; plain text default; `--output json` structured.

Completion criteria:
- `usereport baseline record --name green` followed by `usereport --baseline green` shows non-null `baseline` on at least one signal.
- An injected outlier (z > 3) produces a warn finding; z > 6 produces crit.
- `diff` on identical files produces empty output; diff on perturbed files shows correct deltas.
- Rolling JSONL is pruned correctly at N=24.

### Phase 3 — Direct collectors and delta engine

Tasks:
- Implement delta engine in `src/collector/cpu.rs` (direct `/proc/stat` reads) with 1 s minimum window.
- Implement direct `disk.rs` (`/proc/diskstats`), `network.rs` (`/proc/net/dev`, `/proc/net/snmp`).
- Implement `src/collector/cgroup.rs`: v1/v2 detection, reads `cpu.stat`, `memory.*`, `io.stat`, `pids.current`; add `--cgroup` flag to `Opt`.
- Implement `src/collector/cpufreq.rs` and `src/collector/interrupts.rs`.
- Preference logic: on Linux use direct collector; fall back to parser-based when `/proc` path is absent.

Completion criteria:
- Tool runs end-to-end inside an Alpine container (no `sysstat`) and produces non-empty signals.
- `--cgroup /sys/fs/cgroup/<unit>` produces per-unit memory/cpu signals.
- `cargo test --all-features` still green.

### Phase 4 — Time-sampled collection

Tasks:
- Add `--duration` and `--interval` parsing to `Opt` with mutual-exclusion checks.
- Refactor collectors that `supports_sampling() -> true` to loop `N = floor(duration/interval)+1` times, populate `Signal::samples`.
- Add `SampleStats` computation in `src/baseline/stats.rs`; implement trend via linear regression slope.
- Update predicate evaluator to resolve `.p50`, `.p95`, `.p99`, `.min`, `.max`, `.trend` path suffixes.
- Bare signal ID with samples resolves to `SampleStats::p50` in predicates.
- Document that `--duration` and `--repetitions` are mutually exclusive (clap `conflicts_with`).

Completion criteria:
- `usereport --duration 30s --interval 2s` produces 16 samples per sampled signal.
- A rule referencing `.p95` fires correctly on a stress fixture.
- Markdown output shows trend indicators where signals carry samples.

### Phase 5 — dmesg miner + pattern catalog

Tasks:
- Implement `src/collector/dmesg.rs`: regex-based parser for all 7 event types in Req 19; each event type is a separate `Signal` with boolean or count value.
- Add `src/pattern/mod.rs`: TOML loader, multi-signal correlator; runs after single-rule pass.
- Author the 6 pattern TOML files in `contrib/patterns/`.
- Add `FindingKind::Pattern` to `Finding` struct.

Completion criteria:
- Synthetic dmesg fixture files produce expected signals (OOM → `dmesg.oom_count > 0`, blocked-task → `dmesg.blocked_task_count > 0`, EXT4/XFS error → `dmesg.fs_error_count > 0`).
- TIME_WAIT exhaustion fixture produces the corresponding pattern finding with expected `suggest` commands.
- A partial signal set (missing one required signal) produces no pattern finding.

### Phase 6 — LLM-friendly output + Claude Code skill

Tasks:
- Define `docs/schemas/llm-output-v1.json` (JSON Schema, draft-07).
- Implement `src/llm.rs`: `LlmOutput` struct and serialisation. `LlmHost.hostname` and `LlmHost.kernel` come from the existing `Context` (do not extend `Context::new()` — its infallible contract must be preserved). `LlmHost.cpu_count`, `LlmHost.mem_total_bytes`, and `LlmHost.load_avg_1m` are read from collector-emitted signals (`host.cpu_count`, `host.mem_total_bytes`, `host.load_avg_1m`); a small `host` collector in `src/collector/mod.rs` emits these from `rustix`/`/proc/loadavg`/`/proc/meminfo`.
- Add `OutputType::Llm` and wire `--output llm` in CLI.
- Implement `src/redact.rs`: SHA-256 HMAC redaction for all fields listed in Req 22; `USEREPORT_REDACT_SALT` env var with compile-time constant fallback.
- Author `skills/usereport-analyze/SKILL.md` (input contract, output structure, citation rules, "never fabricate" constraint, version-check logic).
- Build 5 fixture pairs in `skills/usereport-analyze/fixtures/`.

Completion criteria:
- `usereport --output llm | python3 -m jsonschema --instance /dev/stdin docs/schemas/llm-output-v1.json` exits 0.
- `--redact` with same `USEREPORT_REDACT_SALT` on two runs produces identical hashes.
- No raw hostname, IP, MAC, or username appears in any field of the redacted output.
- Skill `SKILL.md` and 5 fixture pairs committed to repo.

### Phase 7 — eBPF opt-in collectors

Tasks:
- Add `bpf` Cargo feature (off by default).
- Implement `src/collector/bpf.rs` (feature-gated): wrappers for `runqlat`, `biolatency`, `tcpretrans`, `execsnoop`, `cachestat`. Each wrapper calls `which::which("<tool>")` at collect time; emits `info` finding if absent.
- Add histogram signal type (p50/p95/p99 as first-class fields in `SampleStats`).
- Allow rules to reference latency percentiles.

Completion criteria:
- On a host with `bpftrace`, `usereport --bpf` produces at least one histogram signal.
- On a host without, `--bpf` produces an `info` finding per missing tool and exits 0.

### Phase 8 — Workload rule packs

Tasks:
- Author 4 workload rule pack TOML files: `contrib/rules/workloads/{postgres,java,nginx,kubelet}.toml`.
- Implement `src/workload/mod.rs`: load named rule pack; no process-list auto-detection.
- Add `--workload <NAME|none>` to `Opt`; merge workload rules with base rules when NAME is given.

Completion criteria:
- `--workload postgres` loads postgres-specific rules; at least one postgres rule fires against a crafted signal fixture.
- `--workload none` (default) produces identical output to omitting the flag.

## Test Scenarios

**Signal identity stability (AC-1)**
- GIVEN a collector runs `collect()` against the same `/proc/stat` fixture file twice
- WHEN signals are collected both times
- THEN every `Signal` has the same `id`, `unit`, and numeric value (within f64 rounding); no two signals share the same `id`.

**Rule predicate match (AC-2)**
- GIVEN `Rule.when = "vmstat.r > host.cpu_count"`, signals `vmstat.r = 8`, `host.cpu_count = 4`
- WHEN the rule engine evaluates
- THEN a `Finding` is produced with `id` equal to the rule's `id`, `severity` equal to the rule's `severity`, `evidence` containing both signal IDs.

**Rule predicate non-match (AC-3)**
- GIVEN the same rule, signals `vmstat.r = 2`, `host.cpu_count = 4`
- WHEN the rule engine evaluates
- THEN no `Finding` is produced for that rule.

**Exit-code behaviour (AC-4)**
- GIVEN `--exit-on=warn` and exactly one `warn` finding, no `crit` findings
- WHEN the binary exits
- THEN exit code is 1.
- AND given `--exit-on=crit` under the same conditions exit code is 0.
- AND given `--exit-on=never` exit code is 0.

**Malformed rule file isolation (AC-5)**
- GIVEN a TOML rule file in `~/.config/usereport/rules.d/bad.toml` with invalid syntax
- WHEN the binary runs
- THEN built-in rules produce findings normally.
- AND a `warn` finding is emitted whose `summary` contains the file path and parse error text.
- AND the binary exits 0 (no other findings).

**Baseline outlier detection (AC-6)**
- GIVEN a baseline where `cpu.iowait_pct` has `p50 = 3.0`, `mad = 0.5`
- WHEN a run reports `cpu.iowait_pct = 42.0` with `--baseline <name>`
- THEN `Signal::baseline.z_score` is a finite value > 3.
- AND a `warn` or `crit` finding citing `cpu.iowait_pct` is emitted.
- AND no such finding is emitted when `cpu.iowait_pct = 3.1` against the same baseline.

**Rule engine determinism (AC-7)**
- GIVEN a fixed `Vec<Signal>` and the bundled rule set
- WHEN the rule engine runs twice on the same input in the same process
- THEN the two `Vec<Finding>` outputs are identical in content and order.

**Pattern requires all constituent signals (AC-8)**
- GIVEN the `time_wait_exhaustion` pattern requires signals `net.tw_count` and `net.connect_failures`
- WHEN only `net.tw_count` is present
- THEN no pattern finding is produced.
- AND when both signals are present exactly one finding is produced.

**Missing binary graceful skip (AC-9)**
- GIVEN a command whose binary is absent from `$PATH`
- WHEN the runner attempts to execute it
- THEN `CommandResult::SkippedMissing` is produced (not `CommandResult::Error`).
- AND an `info` finding names the missing binary.
- AND other commands in the same run complete normally.
- AND the command's section is absent from Markdown/HTML output.

**JSON renderer extension (AC-10)**
- GIVEN a run producing at least one signal, one finding, and one checked-ok signal ID
- WHEN `--output json` is used
- THEN the output is valid JSON with top-level arrays `signals`, `findings`, `checked_ok` alongside `command_results`.
- AND each `findings` entry includes `id`, `severity`, `summary`, `evidence`, and `suggest`.

**Redaction stability**
- GIVEN two runs on the same host with `--redact` and the same `USEREPORT_REDACT_SALT`
- WHEN both `--output llm` documents are produced
- THEN the redacted hostname hashes are equal.
- AND no raw hostname, IP, MAC, or username appears in either output.

**Sampled trend rule**
- GIVEN `--duration 10s --interval 1s` and per-sample values `[1,2,3,4,5,6,7,8,9,10,11]` (linearly rising)
- WHEN trend is computed
- THEN `Trend::Rising` is returned.
- AND a rule with `when = "cpu.load.trend == rising"` fires and its finding evidence cites the signal.

**`--interval` without `--duration` is rejected**
- GIVEN `usereport --interval 5s` (no `--duration`)
- WHEN the binary starts
- THEN it exits 1 with a CLI error message referencing `--duration`.

**Severity-ordered findings**
- GIVEN findings with severity `Warn(id="b")`, `Crit(id="a")`, `Info(id="c")`, `Warn(id="a")`
- WHEN rendered in Markdown
- THEN output order is: `Crit: a`, `Warn: a`, `Warn: b`, `Info: c`.

## Decision Log

**Bake LLM inference into the binary** — Rejected. Breaks determinism; re-introduces network and runtime dependencies.

**Auto-fix / auto-remediation** — Rejected. `suggest` is read-only; blast radius of a wrong action exceeds saved keystrokes.

**Continuous daemon mode / scrape endpoint** — Rejected. Prometheus/node_exporter cover this. Tool stays one-shot.

**SSH / remote / multi-host execution** — Rejected. Local-only; multi-host callers use `pssh`, `ansible`, `parallel`.

**Tightly bind parsers to commands (1:1)** — Rejected. Decoupling `Collector` from `Command` enables same signal IDs from both direct `/proc` reads and tool-output parsing; this is the core architectural bet of v2.

**LLM as source of truth for findings** — Rejected. Rules produce findings; the LLM produces commentary. Confident wrong narrative is worse than no narrative.

**Make LLM skill the default UX** — Rejected. Binary findings must be useful offline without a model.

**Auto-load all user rules regardless of validity** — Rejected. Malformed file must not poison built-ins; failure surfaces as a finding.

**`--repetitions` semantics for time sampling** — Rejected. Back-to-back repeats are a different concept from time-spaced sampling; `--duration` + `--interval` is explicit and parallel to standard tool conventions.

**Hardcode rule thresholds in Rust** — Rejected. Thresholds vary by distro and workload; they belong in TOML.

**`--workload auto` (process-list scan on every run)** — Rejected. Scanning `/proc/*/comm` on every invocation violates KISS and adds implicit overhead. Default is `--workload none`; opt-in is `--workload <NAME>`.

**eBPF detection at startup (meta-package check)** — Rejected. Per-tool detection at collect time (`which::which`) is more flexible and handles partial availability gracefully.

**`usereport explain` in skill only** — Rejected. Binary-only subcommand keeps the feature offline and deterministic; skill may reference explain IDs but does not re-implement content.

**Confidence score for workload detection** — Rejected (YAGNI). No current consumer of the score; workload is explicit by name, not auto-detected.

**`docs/schemas/report-v1.json`** — Removed. No requirement references it; including it creates maintenance debt without a consumer.

**Prometheus / OpenTelemetry export** — Deferred. Out of scope for v2; revisit once the signal/baseline model has stabilised.

## Open Decisions

1. **Skill LLM transport mechanism.** The skill at `skills/usereport-analyze/SKILL.md` reads its input from stdin or a positional file path. The transport between `usereport --output llm` and the skill (piped shell invocation vs. Claude Code built-in) is not dictated by this SDD. The SKILL.md must document both stdin and file-path invocation. Whether the skill ships with an opinionated default model (Claude API) or remains transport-agnostic is a business choice affecting distribution and airgapped use.

## Out of Scope

- SSH / remote execution / multi-host fan-out.
- Continuous daemon mode, scrape endpoints, or push gateways.
- Auto-remediation, auto-fix, or any state-modifying host action.
- Windows support.
- LLM inference inside the binary.
- Persistent multi-host history or centralised storage.
- Built-in alerting, paging, or notification routing.
- Plugin architecture for collectors in other languages.
- BSD-specific collectors beyond what `rustix` provides cross-platform.
- A web UI, hosted view, or report-sharing service.
- Workload rule packs beyond the four named (postgres, java, nginx, kubelet).
- Community rule packs (welcome but not bundled in this SDD).
- `--workload auto` process-list scanning.
