# SDD: More Useful in a Firefight

Status: Ready for Implementation
Original: specs/sdd/more-useful-firefight.md
Refined: 2026-05-02

---

## Overview

`usereport` already collects signals, evaluates rules, and produces findings — but the report is hard to
navigate under pressure. Findings are disconnected from command outputs, missing tools are silently
omitted, numbers appear without threshold context, and the user cannot tell what was checked vs. skipped.
This SDD closes those gaps across five independently shippable phases.

---

## Context & Constraints

- **Language:** Rust, edition 2024, MSRV 1.85; build with `--all-features` (CLI gated behind `bin` feature)
- **Config format:** TOML, parsed into `src/cli/config.rs` structs (`Config`, `Profile`); `Command` struct lives in `src/command.rs`
- **Templates:** minijinja v2; `contrib/html.j2` and `contrib/markdown.j2`; auto-escape disabled
- **Core types:** `Signal`, `Finding`, `Evidence`, `CommandResult` (enum: `Success`, `Failed`, `Timeout`, `Error`, `SkippedMissing`), `AnalysisReport`, `Rule`, `Collector` trait
- **Rule files:** TOML in `contrib/rules/*.toml`, compiled in via `include_str!`
- **`Trend` enum:** already defined in `src/signal.rs:66` as `pub enum Trend { Rising, Falling, Flat }` — do not redefine
- **`checked_ok: Vec<String>`:** already declared with `#[serde(default)]` on `AnalysisReport` in `src/analysis.rs:213` — do not add the field, only populate it
- **`regex` crate:** already in `[dependencies]` — no new dependency needed
- **Logging:** use `log::warn!`, `log::debug!` throughout — `tracing` is NOT in `Cargo.toml`
- **`Command` visibility:** all existing `Command` fields are `pub(crate)`; new fields follow the same pattern with matching accessor and builder methods
- **No new heavy deps** — check existing deps before adding anything
- **YAGNI** — implement only what each phase specifies; no speculative generality
- **Phase ordering:** Phase 3 depends on the `RuleEngine::run` signature change introduced in Phase 2. Implement Phase 2 before Phase 3. Phases 4 and 5 are independent of Phases 3 and 4. However, both Phase 2 and Phase 5 modify `Analysis::run` / `run_diagnostics`; coordinate those changes carefully.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│  Config (TOML)                                                       │
│  Command { install_hint, what_to_look_for, use_dimension, extract }  │
│  Profile { followup }                                                │
└──────────────────────────────────────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│  cli/mod.rs — generate_report                                        │
│  1. Config::from_str → Config::validate (Phase 5)                   │
│  2. Build source_map: HashMap<signal_id, Vec<command_name>>          │
│     by iterating all registered collectors, calling                  │
│     source_commands() on each, mapping signal_id → [cmd_names]       │
└──────────────────────────────────────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│  Analysis::run                                                       │
│  1. Run commands (parallel) → Vec<CommandResult>                     │
│  2. extract_signals() over CommandResult::Success only  (Phase 5)   │
│  3. Collectors → signals                                             │
│  4. Merge extracted + collector signals                              │
│  5. Baseline annotation                                              │
│  6. RuleEngine::run(signals, ctx, source_map)                        │
│       → (findings, checked_ok)                         (Phase 2/3)  │
│  7. PatternEngine::run                                               │
│  8. sort_findings                                                    │
└──────────────────────────────────────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│  generate_report (cli/mod.rs) — post-analysis                        │
│  Compute signal_thresholds: HashMap<signal_id, ThresholdInfo>        │
│  Compute use_coverage: Vec<UseCoverageEntry> (all 12 entries)        │
│  Compute VitalSigns struct                             (Phase 2, 4)  │
└──────────────────────────────────────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│  Template (minijinja)                                                │
│  Vital Signs → USE Coverage → Coverage Gaps → Findings (annotated)  │
│  Evidence commands (promoted, insertion order) → Full output         │
│  (collapsed in <details>) → Healthy → Where next → Diff tip         │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Requirements

### Config & metadata
1. The system shall add an optional `install_hint: Option<String>` field to the `Command` struct in `src/command.rs`.
2. The system shall add an optional `what_to_look_for: Option<String>` field to the `Command` struct in `src/command.rs`.
3. The system shall fix the broken `tcp_mem` link in `contrib/linux.conf` to `https://man7.org/linux/man-pages/man7/tcp.7.html`.
4. The system shall normalize all `http://man7.org` links in `contrib/linux.conf` to `https://`.
5. The system shall update the RFC 793 link in `contrib/linux.conf` to `https://datatracker.ietf.org/doc/html/rfc793`.
6. The system shall fix or add `description` values for: `dmesg` (missing), `vmstat` (inaccurate; use accurate multi-domain description covering virtual memory, CPU, IO, system activity), `free` (add that `-m` means MiB; mention buffers/page cache), `slabinfo` (describe kernel slab allocator cache statistics, not just permissions), `socket_stat` (resolve "cf. man1" and "FRAG: currently unclear" with accurate text) in `contrib/linux.conf`.
7. The system shall fix `vm_stat` link in `contrib/osx.conf` to `https://www.unix.com/man-page/osx/1/vm_stat/`.
8. The system shall update the `vm_stat` description in `contrib/osx.conf` to mention Apple Silicon 16 KB page size.
9. The system shall populate `what_to_look_for` in `contrib/linux.conf` for: `vmstat`, `free`, `iostat`, `sar_cpu`, `sar_q`, `ss_all_tcp`, `socket_stat`, `tcp_mem`.
10. The system shall populate `install_hint = "apt-get install sysstat"` in `contrib/linux.conf` for all `sar_*` variants and `pidstat`.
11. The system shall populate `what_to_look_for` in `contrib/osx.conf` for: `vm_stat`, `iostat`, `memory_pressure`, `netstat_an`, `netstat_i`, `nettop_snapshot`.

### Report completeness
12. The system shall render a **Coverage Gaps** section at the top of the report (above Findings) listing every `SkippedMissing` command by name, including its binary and `install_hint` when present.
13. The system shall include a "findings may be incomplete" notice in the Coverage Gaps section when any commands were skipped.
14. The system shall populate `AnalysisReport.checked_ok` with the IDs of signals that appear in `evidence_ids` of any rule that was evaluated but did not fire (i.e., rule was evaluated, no predicate fired). Return as a sorted `Vec<String>`.
15. The system shall render a **Healthy** section listing `checked_ok` signal IDs when the list is non-empty.
16. The system shall annotate the Signals table with a "vs. threshold" column. For each signal, look up `signal_thresholds[signal.id]`; if present, render `"{signal.observed_value} / {threshold.value} {threshold.severity}"` (e.g., `"15 / 20 warn"`). Signals with no matching simple threshold show a blank cell.

### Diagnostic thread
17. The system shall add a `source_commands: Vec<String>` field to `Evidence`.
18. Each `Collector` implementation shall implement `fn source_commands(&self) -> &[&str]` on the `Collector` trait (default: `&[]`).
19. The rule engine shall populate `Evidence.source_commands` when building findings, using a signal→source-commands map (`source_map: &HashMap<String, Vec<String>>`) passed in from `generate_report`.
20. In HTML output, each Finding's evidence line shall link to the corresponding command section via `href="#cmd-{command-name}"`.
21. In HTML output, command result section headings shall carry `id="cmd-{command-name}"` attributes.
22. In HTML output, command sections that contributed evidence to any finding shall be rendered inline before all other command sections, with a badge "Evidence for: {finding-id}". When one command is evidence for multiple findings, it appears once with a comma-separated badge listing all finding IDs. Evidence commands are ordered by their position in `command_results` (insertion order).
23. In HTML output, remaining non-evidence command sections shall be wrapped in `<details><summary>Full output (N commands)</summary>`.
24. In Markdown output, each Finding's evidence line shall append `(see: {command-name})` for each entry in `source_commands`.
25. In the suggest rendering, when a suggest string exactly equals the `.command` field of a command in `command_results`, the system shall render `<a href="#cmd-{name}">(see: {command.title} ↓)</a>` instead of the raw string in HTML, and append ` (see above)` in Markdown. Matching uses exact string equality.

### At-a-glance overview
26. The system shall render a **Vital Signs** block at the very top of the report (above Coverage Gaps and Findings) with four resource lines: CPU, Memory, Disk, Network.
27. Each Vital Signs resource line shall show the relevant signal values (see Data Models for field→signal_id mapping), a trend arrow (`↑` Rising, `↓` Falling, `→` Flat) when a trend is present on the signal, and a severity badge (`WARN`, `CRIT`) when a finding covers that resource. When Crit and Warn both appear, the line shows `CRIT` (most severe wins; lowest `rank()` value). A resource line shows `[not profiled]` when all its signal fields are `None`.
28. The system shall add an optional `use_dimension: Option<UseDimension>` field to the `Command` struct in `src/command.rs`.
29. The system shall render a **USE Coverage** table after the Vital Signs block, showing all 12 resource/aspect combinations (4 resources × 3 aspects) with covered/absent status.
30. The system shall add an optional `followup: Vec<ProfileFollowup>` field to `Profile` in `src/cli/config.rs`.
31. When findings fire that match a profile's `followup` entries (exact string equality on `Finding.id`), the report shall render a **Where to investigate next** section with the recommended profile and reason.
32. When `findings` is non-empty, the report shall render a tip containing the string `"usereport diff"` at the bottom. When `findings` is empty, no such tip shall appear.

### Output-derived signals
33. The system shall add an optional `extract: Vec<CommandExtract>` array to the `Command` struct in `src/command.rs`.
34. After a command runs with `CommandResult::Success`, the system shall apply all `extract` entries against its stdout, extract numeric values, aggregate them, and emit a `Signal`.
35. Extracted signals shall participate in the rule engine and Signals table on equal footing with collector-produced signals.
36. When extraction finds no matching lines and `aggregate != Count`, the system shall emit nothing and log `log::warn!` with the command name and pattern.
37. When extraction finds a non-numeric `val` capture, the system shall log `log::warn!` with the command name, pattern, and captured value, then continue to the next line.
38. The system shall validate that `extract.pattern` is a valid regex and (unless `aggregate = Count`) contains named group `(?P<val>...)` in `Config::validate`, returning `Result<()>` using the existing module error type. Validation errors use a new `Error::InvalidExtractPattern { command: String, pattern: String, reason: String }` variant.
39. The system shall extend `usereport explain` to accept a command name: look up the command in config by name, render its `title`, `description`, `what_to_look_for`, `extract` entries, and `links`. Fall back to existing rule/signal lookup if no command matches.
40. Multiple `extract` entries on the same command sharing the same `signal_id` are permitted; each entry produces an independent `Signal` emission (last-one-wins is not enforced — both signals are emitted and the rule engine sees both).

---

## File & Module Structure

| File | Change |
|------|--------|
| `src/command.rs` | Add `install_hint`, `what_to_look_for`, `use_dimension`, `extract` fields (all `pub(crate)`); add accessor and builder methods for each; add `UseDimension`, `UseResource`, `UseAspect`, `CommandExtract`, `Aggregate` types here or in `src/cli/config.rs` (co-locate with `Command`) |
| `src/cli/config.rs` | Add `ProfileFollowup` type; add `#[serde(default)] pub followup: Vec<ProfileFollowup>` to `Profile` |
| `src/collector/mod.rs` | Add `fn source_commands(&self) -> &[&str] { &[] }` default method to `Collector` trait |
| `src/collector/cpu.rs` | Implement `source_commands()` (platform-gated) |
| `src/collector/memory.rs` | Implement `source_commands()` (platform-gated) |
| `src/collector/disk.rs` | Implement `source_commands()` |
| `src/collector/network.rs` | Implement `source_commands()` (platform-gated) |
| `src/collector/host.rs` | Implement `source_commands()` |
| `src/finding.rs` | Add `source_commands: Vec<String>` to `Evidence` |
| `src/rule/mod.rs` | Change `RuleEngine::run` to accept `source_map` and return `(Vec<Finding>, Vec<String>)`; expose `fn rules(&self) -> &[Rule]` method if not already present |
| `src/analysis.rs` | Change `run_diagnostics` to return `(Vec<Signal>, Vec<Finding>, Vec<String>)`; update `Analysis::run` to populate `AnalysisReport.checked_ok`; add extracted signals from Phase 5 |
| `src/extract.rs` | New file: `pub fn extract_signals(command_name: &str, stdout: &str, extracts: &[CommandExtract]) -> Vec<Signal>` |
| `src/cli/mod.rs` | Compute `signal_thresholds`, `use_coverage`, `VitalSigns`; build `source_map`; call `Config::validate`; extend `run_explain`; pass all computed values to template context |
| `contrib/linux.conf` | Fix links, fix descriptions, add `what_to_look_for`, `install_hint`, `use_dimension`, `extract` proof-of-concept entries, `followup` on default profile |
| `contrib/osx.conf` | Fix links, fix descriptions, add `what_to_look_for`, `use_dimension` |
| `contrib/html.j2` | Add Vital Signs, USE Coverage, Coverage Gaps, threshold column, evidence promotion/collapse, anchor links, diff tip |
| `contrib/markdown.j2` | Add Vital Signs, USE Coverage, Coverage Gaps, threshold column, source-command refs, diff tip |

### Existing signatures for reference

- `RuleEngine::run` current: `pub fn run(&self, signals: &[Signal], ctx: &CollectCtx) -> Vec<Finding>`
- `run_diagnostics` current (in `src/analysis.rs:147`): `fn run_diagnostics(...) -> (Vec<Signal>, Vec<Finding>)`
- `run_explain` current location: `src/cli/mod.rs` — extend to check command names before rule/signal lookup
- `CollectCtx`: defined in the codebase; unchanged by this SDD — pass through as-is
- Existing test call sites in `tests/sdd_version_2_phase1.rs` (patterns `engine.run(&signals, &ctx)`) must be updated in Phase 2 to destructure the new tuple return.

---

## Data Models

```rust
// src/command.rs additions (or src/cli/config.rs — co-locate with Command)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseDimension {
    pub resource: UseResource,
    pub aspect: UseAspect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UseResource { Cpu, Memory, Disk, Network }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UseAspect { Utilization, Saturation, Errors }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExtract {
    pub pattern: String,        // regex; must contain (?P<val>...) unless aggregate = Count
    pub signal_id: String,
    pub unit: Unit,             // uses existing Unit enum from src/signal.rs; map "ms" → MillisPerOp, "bytes" → BytesPerSec
    pub aggregate: Aggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Aggregate { Last, Max, Min, Avg, Count }

// Additions to existing Command struct in src/command.rs:
// pub(crate) install_hint: Option<String>,
// pub(crate) what_to_look_for: Option<String>,
// pub(crate) use_dimension: Option<UseDimension>,
// #[serde(default)]
// pub(crate) extract: Vec<CommandExtract>,
//
// Add accessor methods: install_hint(), what_to_look_for(), use_dimension(), extract()
// Add builder methods: with_install_hint, with_what_to_look_for, with_use_dimension
// matching the pattern of existing with_title, with_description, with_timeout.
//
// All Option fields: #[serde(skip_serializing_if = "Option::is_none")]


// src/cli/config.rs additions

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileFollowup {
    pub finding: String,    // exact Finding.id to match
    pub recommend: String,  // profile name to recommend
    pub reason: String,     // human-readable reason shown in report
}

// Additions to existing Profile struct in src/cli/config.rs:
// #[serde(default)]
// pub followup: Vec<ProfileFollowup>,


// src/finding.rs — modified Evidence
pub struct Evidence {
    pub signal_id: String,
    pub observed: SignalValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_commands: Vec<String>,   // new in Phase 3; empty until populated by rule engine
}


// src/rule/mod.rs — modified RuleEngine::run signature
// Before: pub fn run(&self, signals: &[Signal], ctx: &CollectCtx) -> Vec<Finding>
// After:
pub fn run(
    &self,
    signals: &[Signal],
    ctx: &CollectCtx,
    source_map: &HashMap<String, Vec<String>>,  // signal_id → command names
) -> (Vec<Finding>, Vec<String>)                // (findings, checked_ok)
// checked_ok: sorted Vec<String> of signal IDs appearing in evidence_ids of evaluated rules
// that did NOT fire. Pass &HashMap::new() in Phase 2 before source_map is populated (Phase 3).

// RuleEngine must expose: pub fn rules(&self) -> &[Rule]
// (add if not already present; used to build signal_thresholds in cli/mod.rs)


// src/analysis.rs — modified run_diagnostics
// Before: fn run_diagnostics(...) -> (Vec<Signal>, Vec<Finding>)
// After:  fn run_diagnostics(...) -> (Vec<Signal>, Vec<Finding>, Vec<String>)
// The Vec<String> is checked_ok, threaded out to Analysis::run → AnalysisReport.checked_ok


// src/cli/mod.rs — template context additions

pub struct ThresholdInfo {
    pub severity: Severity,  // most severe (lowest rank()) threshold for this signal
    pub op: String,           // ">" | ">=" | "<" | "<="
    pub value: f64,           // numeric threshold
}
// signal_thresholds: HashMap<String, ThresholdInfo>
// Key = signal_id. Built by parsing RuleEngine.rules() predicate strings.
// Parser handles predicates of the form: `{signal_id} {op} {number}`
//   where op ∈ {">", ">=", "<", "<="}
// Grammar: <signal_id: no-whitespace token> <op: one of above> <number: parseable as f64>
// Compound predicates (containing "&&", "||", or a token sequence that doesn't match the
//   3-token form above) are silently skipped.
// For a given signal_id, keep only the entry with the lowest Severity::rank() (most severe).
// Signals with only compound predicates show a blank cell in the threshold column.
//
// Severity ordering: Crit = rank 0, Warn = rank 1, Info = rank 2.
// Confirm or add: impl Severity { pub fn rank(&self) -> u8 { ... } }


// Trend is imported from crate::signal::Trend — do NOT redefine

pub struct CpuLine {
    pub load_ratio: Option<f64>,        // signal_id: "cpu.load1_per_core"
    pub iowait_pct: Option<f64>,        // signal_id: "cpu.iowait_pct"
    pub steal_pct: Option<f64>,         // signal_id: "cpu.steal_pct"
    pub trend: Option<Trend>,           // from Signal.trend field if present on any cpu.* signal
    pub severity: Option<Severity>,     // highest severity of findings whose evidence signal_id starts with "cpu."
}

pub struct MemLine {
    pub used_pct: Option<f64>,          // signal_id: "mem.used_pct"
    pub swap_used_pct: Option<f64>,     // signal_id: "mem.swap_used_pct"
    pub severity: Option<Severity>,     // highest severity of findings whose evidence signal_id starts with "mem."
}

pub struct DiskLine {
    pub max_util_pct: Option<f64>,      // signal_id: "disk.util_pct" (max across devices)
    pub max_await_ms: Option<f64>,      // signal_id: "disk.await_ms" (max across devices)
    pub severity: Option<Severity>,     // highest severity of findings whose evidence signal_id starts with "disk."
}

pub struct NetLine {
    pub retrans_pct: Option<f64>,       // signal_id: "net.retrans_pct"
    pub drops: Option<i64>,             // signal_id: "net.drops"
    pub severity: Option<Severity>,     // highest severity of findings whose evidence signal_id starts with "net."
}

pub struct VitalSigns {
    pub cpu: CpuLine,
    pub memory: MemLine,
    pub disk: DiskLine,
    pub network: NetLine,
}
// Signal prefix matching: "cpu." → CpuLine, "mem." → MemLine, "disk." → DiskLine, "net." → NetLine.
// Severity = highest Severity (lowest rank()) among all Finding.evidence entries whose
//   signal_id starts with the resource prefix, across all fired findings.
// When Crit and Warn both present: show CRIT (rank 0 wins).
// When all Option fields for a resource line are None: template renders "[not profiled]".
// VitalSigns is computed in generate_report and stored on AnalysisReport.vital_signs
// (add pub vital_signs: VitalSigns to AnalysisReport for testability).


pub struct UseCoverageEntry {
    pub resource: UseResource,
    pub aspect: UseAspect,
    pub covered: bool,
}
// use_coverage always contains all 12 entries (4 resources × 3 aspects).
// covered = true iff at least one command with that use_dimension did NOT return SkippedMissing.
// Add pub use_coverage: Vec<UseCoverageEntry> to AnalysisReport (for testability).
// Populate in generate_report after analysis completes.
```

---

## Configuration

### New fields on `Command` (in `src/command.rs`)

| Field | Type | Required | Default | Notes |
|-------|------|----------|---------|-------|
| `install_hint` | `String` | No | absent | Human-readable install instruction for the binary |
| `what_to_look_for` | `String` | No | absent | Guidance shown in `explain` and report |
| `use_dimension.resource` | `String` | No (required if `use_dimension` set) | — | One of: `cpu`, `memory`, `disk`, `network` |
| `use_dimension.aspect` | `String` | No (required if `use_dimension` set) | — | One of: `utilization`, `saturation`, `errors` |
| `extract` | Array | No | `[]` | See `CommandExtract` fields below |
| `extract[].pattern` | `String` | Yes | — | Regex; must contain `(?P<val>...)` unless `aggregate = "count"` |
| `extract[].signal_id` | `String` | Yes | — | ID for the emitted `Signal` |
| `extract[].unit` | `String` | Yes | — | One of: `"pct"`, `"count"`, `"ms"`, `"bytes"` — deserialized as `Unit` enum (`"ms"` → `MillisPerOp`, `"bytes"` → `BytesPerSec`, `"pct"` → `Pct`, `"count"` → `Count`) |
| `extract[].aggregate` | `String` | Yes | — | One of: `"last"`, `"max"`, `"min"`, `"avg"`, `"count"` |

### New fields on `[[profile]]` (in `src/cli/config.rs`)

| Field | Type | Required | Default | Notes |
|-------|------|----------|---------|-------|
| `followup` | Array | No | `[]` | See `ProfileFollowup` fields below |
| `followup[].finding` | `String` | Yes | — | Finding ID to match (exact equality on `Finding.id`) |
| `followup[].recommend` | `String` | Yes | — | Profile name to recommend |
| `followup[].reason` | `String` | Yes | — | Human-readable reason shown in report |

### `contrib/linux.conf` additions

`use_dimension` assignments:
- `sar_cpu`, `mpstat` → `{resource = "cpu", aspect = "utilization"}`
- `sar_q`, `vmstat` → `{resource = "cpu", aspect = "saturation"}`
- `pidstat` → `{resource = "cpu", aspect = "utilization"}`
- `free` → `{resource = "memory", aspect = "utilization"}`
- `swapon`, `swap_usage` → `{resource = "memory", aspect = "saturation"}`
- `meminfo`, `slabinfo` → `{resource = "memory", aspect = "utilization"}`
- `iostat` → `{resource = "disk", aspect = "utilization"}`
- `df` → `{resource = "disk", aspect = "utilization"}`
- `sar_dev` → `{resource = "network", aspect = "utilization"}`
- `sar_edev`, `netstat_tcp_s` → `{resource = "network", aspect = "errors"}`
- `sar_tcp`, `ss_all_tcp`, `socket_stat`, `tcp_mem` → `{resource = "network", aspect = "saturation"}`

Default profile `followup` entries:
```toml
[[profile.followup]]
finding = "cpu.iowait_elevated"
recommend = "mem"
reason = "iowait often driven by memory pressure"

[[profile.followup]]
finding = "mem.pressure"
recommend = "mem"
reason = "full memory inspection"

[[profile.followup]]
finding = "net.retransmit_elevated"
recommend = "net"
reason = "TCP retransmits warrant full network investigation"
```

Proof-of-concept `extract` entries:
```toml
# In dmesg command
[[command.extract]]
pattern = 'Out of memory:'
signal_id = "dmesg.oom_count"
unit = "count"
aggregate = "count"

# In vmstat command
[[command.extract]]
pattern = '^\s*(?:\d+\s+){9}\d+\s+(?P<val>\d+)'
signal_id = "vmstat.wa_pct"
unit = "pct"
aggregate = "max"
```

### `contrib/osx.conf` additions

`use_dimension` assignments:
- `vm_stat` → `{resource = "memory", aspect = "utilization"}`
- `memory_pressure` → `{resource = "memory", aspect = "saturation"}`
- `iostat` → `{resource = "disk", aspect = "utilization"}`
- `sar_dev`, `netstat_i`, `nettop_snapshot` → `{resource = "network", aspect = "utilization"}`
- `sar_edev`, `netstat_an` → `{resource = "network", aspect = "errors"}`

### `Collector` trait `source_commands()` return values

| Collector | Platform | Returns |
|-----------|----------|---------|
| `CpuCollector` | Linux | `&["sar_cpu", "mpstat", "iostat"]` |
| `CpuCollector` | macOS | `&["vm_stat", "iostat"]` |
| `MemoryCollector` | Linux | `&["free", "vmstat"]` |
| `MemoryCollector` | macOS | `&["vm_stat", "memory_pressure"]` |
| `DiskCollector` | both | `&["iostat", "df"]` |
| `NetworkCollector` | Linux | `&["sar_dev", "sar_tcp", "sar_edev", "ss_all_tcp", "netstat_tcp_s"]` |
| `NetworkCollector` | macOS | `&["sar_dev", "netstat_i", "nettop_snapshot"]` |
| `HostCollector` | both | `&["uptime"]` |

Command names in the table above must exactly match the `name` fields in `linux.conf`/`osx.conf`. Verify before implementing — any conf rename silently breaks provenance.

### `source_map` construction in `generate_report` (Phase 3)

Use the inversion approach: for each collector, for each signal ID the collector is known to produce (hard-coded per collector in `generate_report`), insert `(signal_id, collector.source_commands().to_vec())` into `source_map: HashMap<String, Vec<String>>`. This mapping is hard-coded in `generate_report`; collectors do not expose a `signal_ids()` method.

---

## Error Handling

| Failure | Trigger | Behaviour | User-visible |
|---------|---------|-----------|--------------|
| Invalid regex in `extract.pattern` | `Config::validate` called after `Config::from_str` in `cli/mod.rs` | Return `Err(...)` using `Error::InvalidExtractPattern { command, pattern, reason }` variant | Error message: `"command '{name}': extract pattern '{pattern}' is not a valid regex: {err}"` |
| Missing named group `(?P<val>...)` in `extract.pattern` (non-Count aggregate) | `Config::validate` | Same error variant | Error message: `"command '{name}': extract pattern '{pattern}' must contain a named group (?P<val>...)"` |
| No lines match extract pattern, aggregate != Count | `extract_signals` at runtime | Return empty vec; `log::warn!("extract: no matches for command={name} pattern={pattern}")` | None (silent to user) |
| Non-numeric `val` capture | `extract_signals` at runtime | Skip that line; `log::warn!("extract: non-numeric capture command={name} pattern={pattern} value={val}")` | None (silent to user) |
| `aggregate = Avg` with zero matched lines | `extract_signals` at runtime | Return empty vec (no division); same `log::warn!` as no-match case | None (silent to user) |
| `SkippedMissing` command | Template rendering | Render in Coverage Gaps section | "Coverage Gaps" heading + command name + binary + `install_hint` if present + "findings may be incomplete" notice |

Note: `Aggregate::Count` always emits a `Signal` even when 0 lines match (emits `SignalValue::I64(0)`). A rule `signal_id > 0` evaluates `I64(0) > 0 = false`; no spurious firing.

---

## Implementation Phases

## Phase 1 — Config Quality Fixes

Add `install_hint` and `what_to_look_for` to `Command`. Fix broken links and weak descriptions in both platform config files.

**Changes:**

- `src/command.rs`:
  - Add `pub(crate) install_hint: Option<String>` with `#[serde(skip_serializing_if = "Option::is_none")]`.
  - Add `pub(crate) what_to_look_for: Option<String>` with the same serde attribute.
  - Add accessor methods `install_hint(&self) -> Option<&str>` and `what_to_look_for(&self) -> Option<&str>`.
  - Add builder methods `with_install_hint(mut self, v: impl Into<String>) -> Self` and `with_what_to_look_for(mut self, v: impl Into<String>) -> Self` matching the pattern of `with_title`, `with_description`, `with_timeout`.
- `contrib/linux.conf`:
  - Fix `tcp_mem` link to `https://man7.org/linux/man-pages/man7/tcp.7.html`.
  - Normalize all remaining `http://man7.org` links to `https://`.
  - Update RFC 793 link to `https://datatracker.ietf.org/doc/html/rfc793`.
  - Add description to `dmesg`.
  - Replace `vmstat` description with accurate multi-domain text covering virtual memory, CPU, IO, system activity.
  - Update `free` description: add that `-m` means MiB, mention buffers/page cache distinction.
  - Update `slabinfo` description: describe kernel slab allocator cache statistics, not just permissions.
  - Update `socket_stat` description: resolve "cf. man1" and "FRAG: currently unclear" with accurate text.
  - Populate `what_to_look_for` for: `vmstat`, `free`, `iostat`, `sar_cpu`, `sar_q`, `ss_all_tcp`, `socket_stat`, `tcp_mem`.
  - Populate `install_hint = "apt-get install sysstat"` for all `sar_*` variants and `pidstat`.
- `contrib/osx.conf`:
  - Fix `vm_stat` link to `https://www.unix.com/man-page/osx/1/vm_stat/`.
  - Update `vm_stat` description to mention Apple Silicon 16 KB page size.
  - Populate `what_to_look_for` for: `vm_stat`, `iostat`, `memory_pressure`, `netstat_an`, `netstat_i`, `nettop_snapshot`.

Phase complete when:
- `cargo test --all-features` passes.
- `cargo check --all-features` passes.
- Both config files parse without error (`Config::from_str` returns `Ok`).
- All `links` URL strings in `linux.conf` start with `https://`.
- The `tcp_mem` entry links to a URL containing `man7/tcp.7.html`.
- All named commands (`vmstat`, `free`, `iostat`, `sar_cpu`, `sar_q`, `ss_all_tcp`, `socket_stat`, `tcp_mem`, `vm_stat`, `memory_pressure`, `netstat_an`, `netstat_i`, `nettop_snapshot`) have a non-empty `what_to_look_for` when the config is parsed.

### Test Scenarios

**C1** GIVEN `linux.conf` is loaded WHEN `Config::from_str` parses it THEN it returns `Ok` and every `[[command]]` entry that has `install_hint` set has a non-empty string value.

**C2** GIVEN a `[[command]]` block with `install_hint = "apt-get install sysstat"` and `what_to_look_for = "look for high wa"` WHEN `Config::from_str` parses it THEN both fields deserialize to their string values.

**C3** GIVEN `linux.conf` WHEN all `links` URL strings are collected via TOML parse THEN none starts with `"http://"` and the `tcp_mem` entry's links contain a URL with the substring `"man7/tcp.7.html"`.

**C4** GIVEN a `Command` struct where `install_hint` and `what_to_look_for` are both `None` WHEN serialized with `toml::to_string_pretty` THEN neither key appears in the output.

**C5** GIVEN `osx.conf` WHEN `Config::from_str` parses it THEN it returns `Ok`.

**C6** GIVEN `linux.conf` WHEN `Config::from_str` parses it THEN it returns `Ok`.

---

## Phase 2 — Report Completeness

Surface what was skipped, what was healthy, and add threshold context to the Signals table.

**Dependencies:** none.

**Changes:**

- `src/rule/mod.rs`:
  - Change `RuleEngine::run` from:
    `pub fn run(&self, signals: &[Signal], ctx: &CollectCtx) -> Vec<Finding>`
    to:
    `pub fn run(&self, signals: &[Signal], ctx: &CollectCtx, source_map: &HashMap<String, Vec<String>>) -> (Vec<Finding>, Vec<String>)`
  - `checked_ok` (second return value): sorted `Vec<String>` of signal IDs that appear in `evidence_ids` of any evaluated rule that did NOT fire. A rule is "evaluated" when its predicate is run against the signal set; it "did not fire" when no evidence predicate matched.
  - The `source_map` parameter is accepted here; in Phase 2 callers pass `&HashMap::new()`.
  - Add `pub fn rules(&self) -> &[Rule]` to `RuleEngine` if not already present.
- `src/analysis.rs`:
  - Change `run_diagnostics` from `-> (Vec<Signal>, Vec<Finding>)` to `-> (Vec<Signal>, Vec<Finding>, Vec<String>)`.
  - Update `Analysis::run` to destructure the new tuple and set `AnalysisReport.checked_ok`.
  - Pass `&HashMap::new()` as `source_map` in Phase 2; Phase 3 replaces this with the real map.
- `tests/sdd_version_2_phase1.rs`:
  - Update all `engine.run(&signals, &ctx)` call sites to `engine.run(&signals, &ctx, &HashMap::new())` and destructure the tuple.
- `src/cli/mod.rs` (`generate_report`):
  - Build `signal_thresholds: HashMap<String, ThresholdInfo>` by calling `rule_engine.rules()` and parsing each rule's `when` predicate string.
  - Predicate grammar: `{signal_id} {op} {number}` where `op` ∈ `{">", ">=", "<", "<="}`. Any predicate that doesn't match this 3-token form (contains `&&`, `||`, or has a different structure) is silently skipped.
  - For each signal_id, keep only the entry with the lowest `Severity::rank()` (most severe threshold).
  - Pass `signal_thresholds` into the template context.
- `contrib/html.j2` and `contrib/markdown.j2`:
  - Add **Coverage Gaps** section before Findings: iterate `command_results` for `SkippedMissing` variants. Show command name, binary, and `install_hint` when present. Include "findings may be incomplete" when list is non-empty.
  - Add **Healthy** section after Findings when `checked_ok` is non-empty: list signal IDs.
  - Add "vs. threshold" column to Signals table: for each signal row, render `"{signal.observed_value} / {threshold.value} {threshold.severity}"` if `signal_thresholds[signal.id]` exists, else blank.

Phase complete when:
- A report generated with a command whose binary is absent shows a "Coverage Gaps" section listing the command name.
- A report with `install_hint` on a skipped command shows that hint in the Coverage Gaps section.
- The Coverage Gaps section shows a "findings may be incomplete" notice when any command was skipped.
- A report with no issues shows a "Healthy" section listing signals checked OK.
- The Signals table renders a "vs. threshold" column where populated rows contain the numeric threshold and severity label (e.g., `"15 / 20 warn"`).
- A Markdown report also includes Coverage Gaps, Healthy, and threshold column.

### Test Scenarios

**C1** GIVEN a config where `sar_cpu`'s binary is not present in PATH WHEN a report is generated THEN the rendered output contains the string `"Coverage Gaps"` and the substring `"sar_cpu"`.

**C2** GIVEN `sar_cpu` has `install_hint = "apt-get install sysstat"` and its binary is absent WHEN the report renders THEN the Coverage Gaps section contains `"apt-get install sysstat"`.

**C3** GIVEN `sar_cpu` is skipped WHEN the Coverage Gaps section renders THEN the output contains a "findings may be incomplete" notice.

**C4** GIVEN `signals = [Signal { id: "cpu.iowait_pct", value: 5.0 }]` and one rule that fires only when `cpu.iowait_pct > 20` WHEN `RuleEngine::run` is called with `source_map = &HashMap::new()` THEN the returned tuple has `findings` empty and `checked_ok` contains `"cpu.iowait_pct"`.

**C5** GIVEN `cpu.iowait_pct = 25.0` and the same rule WHEN `RuleEngine::run` is called THEN `findings` contains one entry and `checked_ok` does NOT contain `"cpu.iowait_pct"`.

**C6** GIVEN `checked_ok = ["cpu.iowait_pct"]` in `AnalysisReport` WHEN the report renders THEN the output contains the string `"Healthy"` and `"cpu.iowait_pct"`.

**C7** GIVEN a rule predicate `cpu.iowait_pct > 20` labelled warn and signal value `15.0` WHEN the Signals table renders THEN the threshold column row for `cpu.iowait_pct` contains `"20"` and `"warn"`.

---

## Phase 3 — Diagnostic Thread

Connect findings to the command outputs that show them.

**Dependencies:** Phase 2 must be complete (uses updated `RuleEngine::run` signature).

**Changes:**

- `src/finding.rs`:
  - Add `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub source_commands: Vec<String>` to `Evidence`.
  - Update `Evidence` construction sites to include `source_commands: Vec::new()`.
- `src/collector/mod.rs`:
  - Add `fn source_commands(&self) -> &[&str] { &[] }` as a default method to the `Collector` trait.
- `src/collector/cpu.rs`: implement `source_commands()`:
  - Linux (`#[cfg(target_os = "linux")]`): `&["sar_cpu", "mpstat", "iostat"]`
  - macOS (`#[cfg(target_os = "macos")]`): `&["vm_stat", "iostat"]`
- `src/collector/memory.rs`: implement `source_commands()`:
  - Linux: `&["free", "vmstat"]`
  - macOS: `&["vm_stat", "memory_pressure"]`
- `src/collector/disk.rs`: implement `source_commands()` returning `&["iostat", "df"]` (both platforms).
- `src/collector/network.rs`: implement `source_commands()`:
  - Linux: `&["sar_dev", "sar_tcp", "sar_edev", "ss_all_tcp", "netstat_tcp_s"]`
  - macOS: `&["sar_dev", "netstat_i", "nettop_snapshot"]`
- `src/collector/host.rs`: implement `source_commands()` returning `&["uptime"]`.
- `src/cli/mod.rs` (`generate_report`):
  - Build `source_map: HashMap<String, Vec<String>>` using the inversion approach: hard-code signal_id → command_names mappings per collector. For each collector, for each signal ID that collector produces, insert `(signal_id, collector.source_commands().to_vec())` into the map. This mapping is maintained in `generate_report`, not via a trait method.
  - Replace the `&HashMap::new()` placeholder from Phase 2 with the real `source_map`.
  - Pass `&source_map` to `RuleEngine::run`.
- `src/rule/mod.rs`:
  - After building each `Evidence`, look up `source_map.get(&evidence.signal_id)` and clone into `evidence.source_commands`.
- `contrib/html.j2`:
  - For each finding's evidence entry, render `<a href="#cmd-{name}">{name}</a>` for each `source_commands` entry.
  - For each command result section heading, add `id="cmd-{command.name}"`.
  - After the Findings section, render promoted **Evidence Commands** sub-section: for commands referenced in any finding's evidence, render in `command_results` insertion order. When one command is evidence for multiple findings, render it once with a badge listing all finding IDs (comma-separated): `"Evidence for: {id1}, {id2}"`.
  - Wrap all remaining non-evidence command sections in `<details><summary>Full output (N commands)</summary>`.
  - In suggest rendering: when a suggest string exactly equals a command's `.command` field in `command_results`, render `<a href="#cmd-{name}">(see: {command.title} ↓)</a>` instead of the raw string.
- `contrib/markdown.j2`:
  - Add `(see: {name})` after evidence signal IDs when `source_commands` is non-empty.
  - In suggest rendering: append ` (see above)` when suggest string exactly equals a command's `.command` field.

Phase complete when:
- A finding with `source_commands = ["iostat"]` renders `href="#cmd-iostat"` in the HTML evidence line.
- The iostat section heading has `id="cmd-iostat"` in HTML.
- Evidence command sections appear before non-evidence sections in HTML (in `command_results` insertion order).
- Non-evidence commands are wrapped in `<details>` in HTML.
- A Markdown report with a finding that has `source_commands` renders `(see: command-name)` inline.
- A Markdown report where `suggest` exactly matches a run command's `.command` renders `(see above)`.
- A suggest item whose string exactly matches a run command's `.command` renders a cross-reference rather than a bare command string in HTML.

### Test Scenarios

**C1** GIVEN `source_map = {"cpu.iowait_pct": ["sar_cpu"]}` and a rule fires with evidence signal `cpu.iowait_pct` WHEN `RuleEngine::run` builds the Finding THEN `finding.evidence[0].source_commands` equals `["sar_cpu"]`.

**C2** GIVEN `Evidence { signal_id: "disk.util_pct", source_commands: ["iostat"] }` WHEN the HTML template renders the finding THEN the evidence line contains `href="#cmd-iostat"`.

**C3** GIVEN a command named `iostat` in `command_results` WHEN the HTML template renders the command section heading THEN the heading element contains `id="cmd-iostat"`.

**C4** GIVEN `command_results` contains evidence commands `["iostat", "sar_dev"]` and non-evidence commands `["df", "free"]` WHEN the HTML template renders THEN the `iostat` and `sar_dev` sections appear before the `<details>` block, and `df` and `free` appear inside `<details>`.

**C5** GIVEN Markdown output and `evidence[0].source_commands = ["iostat"]` WHEN the template renders THEN the evidence line contains the substring `"(see: iostat)"`.

**C6** GIVEN `suggest = ["iostat -x 1 5"]` on a finding AND `command_results` contains a command with `.command = "iostat -x 1 5"` WHEN the HTML template renders the suggest list THEN the output contains `href="#cmd-iostat"` and does not contain the bare string `"iostat -x 1 5"` outside an anchor tag.

---

## Phase 4 — At-a-Glance Overview

Add the Vital Signs header, USE coverage map, and profile follow-up recommendations.

**Dependencies:** none (can be implemented in parallel with Phase 3).

**Changes:**

- `src/command.rs`:
  - Add `pub(crate) use_dimension: Option<UseDimension>` to `Command`.
  - Add accessor `use_dimension(&self) -> Option<&UseDimension>` and builder `with_use_dimension(mut self, v: UseDimension) -> Self`.
- `src/cli/config.rs`:
  - Add `#[serde(default)] pub followup: Vec<ProfileFollowup>` to `Profile`.
- `src/analysis.rs` / `src/cli/mod.rs`:
  - Add `pub use_coverage: Vec<UseCoverageEntry>` and `pub vital_signs: VitalSigns` to `AnalysisReport`.
  - Compute `use_coverage` after analysis completes: for each of the 12 `(UseResource, UseAspect)` pairs, set `covered = true` if at least one command with that `use_dimension` returned a non-`SkippedMissing` result.
  - Compute `VitalSigns` in `generate_report`:
    - For each resource line, look up signal values by exact signal_id from the report's signal list.
    - Severity = highest (lowest `rank()`) `Severity` among all `Finding.evidence` entries whose `signal_id` starts with the resource prefix (`"cpu."`, `"mem."`, `"disk."`, `"net."`).
    - Trend: use `Signal.trend` field from `crate::signal::Trend` if present; otherwise `None`.
    - When all `Option` fields for a resource line are `None`, the template renders `[not profiled]`.
  - Pass `vital_signs` and `use_coverage` into the template context (via `AnalysisReport`).
- `contrib/linux.conf`: populate `use_dimension` per the Configuration section. Populate `followup` on default profile per the Configuration section.
- `contrib/osx.conf`: populate `use_dimension` per the Configuration section.
- `contrib/html.j2` and `contrib/markdown.j2`:
  - Render Vital Signs block as first content after the report title.
  - Render USE Coverage table (4 rows × 3 columns) after Vital Signs.
  - Render **Where to investigate next** section after Findings: iterate `profile.followup`, filter to entries whose `finding` matches a fired finding ID (exact string equality on `Finding.id`), render `recommend` and `reason`.
  - Render diff tip at bottom when `findings` is non-empty: include the string `"usereport diff"`.

Phase complete when:
- A report with signals present renders a Vital Signs block with all four resource lines.
- A report where all network commands returned `SkippedMissing` shows `[not profiled]` on the Network line.
- The USE Coverage table marks `{network, saturation}` as covered when `sar_tcp` ran successfully.
- A default-profile report with `cpu.iowait_elevated` fired shows the `mem` follow-up recommendation.
- A report with non-empty findings renders the diff tip containing "usereport diff".
- A report with empty findings does NOT contain "usereport diff".

### Test Scenarios

**C1** GIVEN `cpu.iowait_pct = 23.4` in signals AND a finding with `evidence[].signal_id = "cpu.iowait_pct"` and severity Warn WHEN `VitalSigns` is computed THEN `cpu.iowait_pct == 23.4` and `cpu.severity == Warn`.

**C2** GIVEN `VitalSigns.cpu.iowait_pct = 23.4` and `cpu.severity = Warn` WHEN the report renders THEN the CPU vital signs line contains `"23"` and `"WARN"`.

**C3** GIVEN all commands with `use_dimension.resource = "network"` returned `SkippedMissing` WHEN `use_coverage` is computed THEN all 3 entries with `resource = Network` have `covered = false`.

**C4** GIVEN `use_coverage` has `{Network, Saturation, covered: false}` and all network signal fields are `None` WHEN the report renders THEN the Network vital signs line contains `"[not profiled]"`.

**C5** GIVEN `sar_tcp` ran successfully and has `use_dimension = {resource: "network", aspect: "saturation"}` WHEN `use_coverage` is computed THEN the entry `{Network, Saturation}` has `covered = true`.

**C6** GIVEN the default Linux profile and a fired finding with `id = "cpu.iowait_elevated"` WHEN the report renders THEN a section appears containing `"mem"` and the reason string `"iowait often driven by memory pressure"`.

**C7** GIVEN `findings` is non-empty WHEN the report renders THEN the output contains the string `"usereport diff"`.

**C8** GIVEN `findings` is empty WHEN the report renders THEN the output does NOT contain the string `"usereport diff"`.

---

## Phase 5 — Output-Derived Signals

Let command outputs feed the rule engine.

**Dependencies:** none (can be implemented in parallel with Phases 3 and 4). Coordinate with Phase 2 when modifying `Analysis::run` / `run_diagnostics`.

**Changes:**

- `src/command.rs`:
  - Add `#[serde(default)] pub(crate) extract: Vec<CommandExtract>` to `Command`.
  - Add accessor `extract(&self) -> &[CommandExtract]`.
- `src/cli/config.rs`:
  - Add `pub fn validate(&self) -> Result<()>` to `Config` (using the existing module error type).
  - Add `Error::InvalidExtractPattern { command: String, pattern: String, reason: String }` variant to the module error type.
  - Validation iterates all commands, and for each `extract` entry checks:
    1. `extract.pattern` is a valid regex (compile with `regex::Regex::new`).
    2. Unless `extract.aggregate == Aggregate::Count`, the pattern contains the substring `(?P<val>`.
  - Collect all errors; return `Ok(())` if none, else return the first error (or use `miette`/existing error aggregation if available).
- `src/extract.rs` (new file):
  ```rust
  use crate::cli::config::{CommandExtract, Aggregate};
  use crate::signal::{Signal, SignalValue};

  pub fn extract_signals(
      command_name: &str,
      stdout: &str,
      extracts: &[CommandExtract],
  ) -> Vec<Signal>
  ```
  - For each `CommandExtract`:
    - Compile `pattern` to a `Regex` (already validated; use `expect` with a clear message).
    - For `Aggregate::Count`: count matching lines; always emit `Signal { id: extract.signal_id, value: SignalValue::I64(count), unit: extract.unit, at: chrono::Local::now(), .. }` (emits even for 0 matches).
    - For other aggregates: collect `val` capture group from each matching line, parse as `f64`. Skip non-numeric captures with `log::warn!("extract: non-numeric capture command={command_name} pattern={pattern} value={val}")`. Apply aggregate (`Max`, `Min`, `Avg`, `Last`). If result set is empty (no matches or all non-numeric), emit nothing and `log::warn!("extract: no matches for command={command_name} pattern={pattern}")`. `Avg` with zero elements emits nothing (no division by zero).
    - `Signal.at` = `chrono::Local::now()` (not `Utc::now()`).
    - `Signal.id` = `extract.signal_id.clone()`.
    - `Signal.unit` = `extract.unit` (mapped from `CommandExtract.unit`).
- `src/analysis.rs` (`Analysis::run`):
  - After running commands and before running collectors, iterate `CommandResult::Success` results.
  - For each, call `extract_signals(&command.name(), result.stdout(), &command.extract())`.
  - Append returned signals to the signal pool before the rule engine runs.
- `src/cli/mod.rs`:
  - After `Config::from_str`, call `config.validate().into_diagnostic()?` (consistent with existing error handling).
  - In `run_explain`: before the existing rule/signal lookup, check if the argument matches a command `name` field in the loaded config. If found, render: `title`, `description`, `what_to_look_for` (if set), `extract` entries (pattern, signal_id, unit, aggregate), and `links`. If not found, fall back to existing rule/signal lookup.
- `contrib/linux.conf`:
  - Add proof-of-concept `extract` entries to `dmesg` and `vmstat` commands (see Configuration section).

Phase complete when:
- `dmesg.oom_count` appears in the signals list when `dmesg` output contains OOM lines.
- An invalid regex in `[[command.extract]]` causes `Config::validate` to return an error identifying the command and pattern.
- A missing `(?P<val>...)` group (non-Count aggregate) causes `Config::validate` to return an error.
- `usereport explain vmstat` renders the command's `title`, `description`, and `what_to_look_for`.
- Extracted signals trigger rules — a rule `dmesg.oom_count > 0` with severity Crit fires on the extracted signal.

### Test Scenarios

**C1** GIVEN stdout `"Out of memory: Killed process 123\n"` and `extract = [{pattern: "Out of memory:", signal_id: "dmesg.oom_count", unit: "count", aggregate: "count"}]` WHEN `extract_signals` is called THEN it returns exactly one `Signal` with `id = "dmesg.oom_count"` and integer value `1`.

**C2** GIVEN three stdout lines with `val` captures `"5"`, `"18"`, `"12"` and `aggregate = "max"` WHEN `extract_signals` is called THEN the returned signal has value `18.0`.

**C3** GIVEN `aggregate = "count"` and a pattern that matches 3 lines WHEN `extract_signals` is called THEN the returned signal has value `3`.

**C4** GIVEN three matching lines with `val` captures `"4"`, `"6"`, `"8"` and `aggregate = "avg"` WHEN `extract_signals` is called THEN the returned signal has value `6.0`.

**C5** GIVEN three matching lines and `aggregate = "last"` WHEN `extract_signals` is called THEN the returned signal value equals the last parsed capture.

**C6** GIVEN `extract.pattern = "["` (invalid regex) on command `vmstat` WHEN `Config::validate` runs THEN it returns `Err` containing a message with `"vmstat"` and `"["`.

**C7** GIVEN `extract.pattern = "\\d+"` with no `(?P<val>...)` group and `aggregate = "max"` WHEN `Config::validate` runs THEN it returns `Err` containing the command name and the pattern.

**C8** GIVEN stdout with zero matching lines and `aggregate = "max"` WHEN `extract_signals` is called THEN it returns an empty vec and does not panic.

**C9** GIVEN `dmesg.oom_count` is extracted with value `2` AND a rule `dmesg.oom_count > 0` severity Crit WHEN the rule engine runs THEN a Crit finding fires for `dmesg.oom_count`.

**C10** GIVEN `vmstat` is a command in the loaded config with `title`, `description`, and `what_to_look_for` set WHEN `usereport explain vmstat` runs THEN the output contains all three field values.

---

## Decision Log

| Decision | Alternatives considered | Reason chosen |
|----------|------------------------|---------------|
| `what_to_look_for` as a config field | Separate `contrib/knowledge/*.toml` files | Co-located with the command it describes; no extra loading logic. |
| Signal provenance via `source_commands` on `Evidence` | On `Finding` directly | Evidence is the unit that carries signal data; provenance is per-signal. Multiple evidence entries in one finding may point to different commands. |
| `[[command.extract]]` with named group `val` | Column-index extraction; multiple named groups | Named group is readable and self-documenting. Multiple groups add complexity without a current use case (YAGNI). |
| Threshold index built in Rust, passed to template | Compute in Jinja2 template | Avoids duplicating regex/parsing in template. Compound predicates (containing `&&`, `||`, or not matching the 3-token `signal_id op number` form) are silently skipped. |
| Vital Signs computed in Rust (`generate_report`) | Pure Jinja2 template logic | Template logic for selecting the right signals per resource would be complex and untestable. |
| HTML `<details>` for non-evidence command collapse | JavaScript toggle; always expanded | No JS, works everywhere, degrades gracefully. |
| `followup` on `[[profile]]` | On `[[rule]]` | Profile authors know their domain. Avoids coupling rule files to profile names. |
| VitalSigns signal selection via prefix matching | Explicit `resource` annotation on `Signal` | Zero config; works immediately; prefixes (`cpu.`, `mem.`, `disk.`, `net.`) are stable. |
| `explain` command extension included in Phase 5 | Deferred to separate phase | The `explain` extension directly surfaces `what_to_look_for` and `extract` metadata added in Phase 5; shipping them together is coherent. |
| `source_map` hard-coded in `generate_report` | `fn signal_ids()` on `Collector` trait | Collectors don't expose signal IDs programmatically today; hard-coding avoids adding a trait method for metadata that's stable per collector. |
| Suggest cross-reference uses exact string equality | Prefix match; binary-name match | Exact equality is deterministic and testable; no false matches. |
| `VitalSigns` and `use_coverage` added to `AnalysisReport` | Computed only in `generate_report` locals | Putting them on `AnalysisReport` enables unit testing without going through the full render pipeline. |
| `Config::validate` returns `Result<()>` (module error type) | `Result<(), Vec<String>>` | Consistent with existing `Config::from_str` error handling; `Error::InvalidExtractPattern` variant carries all needed context. |
| `Signal.at` uses `chrono::Local::now()` | `chrono::Utc::now()` | `Signal.at` is `DateTime<Local>`; using `Local::now()` avoids a type mismatch. |
| `"ms"` → `MillisPerOp`, `"bytes"` → `BytesPerSec` | Add new `Bytes`/`Millis` variants to `Unit` | `Unit` enum already has `MillisPerOp` and `BytesPerSec`; mapping avoids an additive change to a shared enum. |
| Evidence command appears once when in multiple findings | Once per finding | Reduces duplication; badge lists all finding IDs comma-separated. |

---

## Open Decisions

**1. `generate_report` internal structure**
Reqs 19/22/25 push VitalSigns computation, USE coverage, threshold index, and followup matching all into `cli/mod.rs::generate_report`. The SDD does not prescribe a sub-module boundary.
- Option A: extract a `src/report_context.rs` module with named helper functions (`build_vital_signs`, `build_use_coverage`, `build_threshold_index`, `match_followups`).
- Option B: keep in `generate_report` with named private helper functions in the same file.
Both are correct; the choice affects only internal organization. A coding agent may pick either.

---

## Out of Scope

- Line-level pattern highlighting in HTML (regex → colored lines): orthogonal; can be added after Phase 4.
- Per-command sparklines (signal history over baseline records): requires baseline UI changes beyond this design.
- Interactive TUI mode: entirely separate rendering path.
- Automatic `diff` comparison between two consecutive runs: separate UX flow.
- Workload-specific `what_to_look_for` content (postgres, java, nginx): content work, not architecture; add to config files independently.
- Validating `ProfileFollowup.recommend` against configured profile names at parse time: the field is a free string; an unknown name is a silent no-op.
