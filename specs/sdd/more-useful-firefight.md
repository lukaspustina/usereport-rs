# SDD: More Useful in a Firefight

Status: Draft
Created: 2026-05-02

---

## Overview

`usereport` already collects signals, evaluates rules, and produces findings — but the report is hard to
navigate under pressure. Findings are disconnected from the command outputs that show them. Missing tools
are silently omitted. Numbers appear without context. The user cannot tell what the report checked vs.
what it skipped. This SDD closes those gaps across five independently shippable phases.

---

## Context & Constraints

- **Language:** Rust, edition 2024, MSRV 1.85; build with `--all-features` (CLI gated behind `bin` feature)
- **Config format:** TOML, parsed into `cli/config.rs` structs (`Config`, `Profile`, `Command`)
- **Templates:** minijinja v2; `contrib/html.j2` and `contrib/markdown.j2`; auto-escape disabled
- **Core types:** `Signal`, `Finding`, `Evidence`, `CommandResult` (enum: `Success`, `Failed`, `Timeout`,
  `Error`, `SkippedMissing`), `AnalysisReport`, `Rule`
- **Rule files:** TOML in `contrib/rules/*.toml`, compiled in via `include_str!`
- **No new heavy deps** — check existing deps before adding anything
- **YAGNI** — implement only what each phase specifies; no speculative generality
- **Tests** — failing test before fix for bug fixes; test scenarios listed per phase

---

## Requirements

### Config & metadata
1. The system shall add an optional `install_hint: Option<String>` field to `[[command]]` config.
2. The system shall add an optional `what_to_look_for: Option<String>` field to `[[command]]` config.
3. The system shall fix all broken/incorrect man page links in `contrib/linux.conf` and `contrib/osx.conf`.
4. The system shall fix all weak, misleading, or missing `description` values in both config files.
5. The system shall normalize all `http://man7.org` links to `https://`.

### Report completeness
6. The system shall render a **Coverage Gaps** section at the top of the report (above Findings) listing
   every `SkippedMissing` command by name, including its binary and `install_hint` when present.
7. The system shall include a "findings may be incomplete" notice when any commands were skipped.
8. The system shall populate `AnalysisReport.checked_ok` with the IDs of signals that were evaluated by
   at least one rule and found within bounds (i.e. no rule fired for that signal).
9. The system shall render a **Healthy** section listing `checked_ok` signal IDs when the list is non-empty.
10. The system shall annotate the Signals table with a "vs. threshold" column derived from the compiled
    rule set, showing the signal's warn/crit threshold and whether the current value is within bounds.

### Diagnostic thread
11. The system shall add a `source_commands: Vec<String>` field to `Evidence`.
12. Each `Collector` implementation shall declare the command names (from config) whose output shows the
    same data it measures, via a new `fn source_commands(&self) -> &[&str]` method on the `Collector` trait.
13. The rule engine shall populate `Evidence.source_commands` when building findings.
14. In HTML output, each Finding's evidence shall link (via anchor) to the corresponding command output section.
15. In Markdown/terminal output, each Finding's evidence shall include a `(see: <command-name>)` reference.
16. When rendering `Finding.suggest`, the system shall detect which suggest strings match a command already
    run in the report and replace "run X" with a "see [X output ↓]" reference instead of an action item.
17. In HTML output, command output sections that contributed evidence to a finding shall be promoted above
    all non-evidence sections.
18. In HTML output, non-evidence command sections shall be rendered inside a `<details><summary>` collapse.

### At-a-glance overview
19. The system shall render a **Vital Signs** block at the very top of the report (above Coverage Gaps and
    Findings) summarising CPU, Memory, Disk, and Network in four compact lines, derived from the `signals`
    array and `findings` list.
20. Each Vital Signs line shall include a trend arrow (`↑` Rising, `↓` Falling, `→` Flat) for signals with
    non-flat trends, and a severity badge (`WARN`, `CRIT`) when a finding covers that resource.
21. A resource line shall show `[not profiled]` when no command in the current profile covers that resource.
22. The system shall add an optional `use_dimension: { resource: String, aspect: String }` field to
    `[[command]]` config, with `resource` ∈ {`cpu`, `memory`, `disk`, `network`} and
    `aspect` ∈ {`utilization`, `saturation`, `errors`}.
23. The system shall render a **USE Coverage** table in the report showing which resource/aspect combinations
    are covered by the current profile and which are absent.
24. The system shall add an optional `followup: Vec<{finding: String, recommend: String, reason: String}>`
    field to `[[profile]]` config.
25. When findings fire that match a profile's `followup` entries, the report shall render a
    **Where to investigate next** section with the recommended profile and reason.
26. When findings of Warn or Crit severity are present, the report shall render a tip showing the
    `usereport diff` command for comparing before/after states.

### Output-derived signals
27. The system shall add an optional `[[command.extract]]` array to `[[command]]` config, with fields:
    `pattern: String` (regex with named capture group `val`), `signal_id: String`, `unit: String`,
    `aggregate: String` (one of `last`, `max`, `min`, `avg`, `count`).
28. After a command runs successfully, the system shall apply all `extract` entries against its stdout,
    extract the numeric value(s), aggregate them per the `aggregate` field, and emit a `Signal`.
29. Extracted signals shall participate in the rule engine, baseline comparison, and Signals table on equal
    footing with collector-produced signals.
30. When extraction fails (no matches, non-numeric capture), the system shall log a warning and continue;
    extraction failures shall never fail the run.

---

## Implementation Phases

## Phase 1 — Config Quality Fixes

Fix broken links, weak descriptions, and add the two new metadata fields to the config types and both
platform config files.

**Changes:**

- `cli/config.rs`: add `install_hint: Option<String>` and `what_to_look_for: Option<String>` to the
  `Command` struct and implement serde `skip_serializing_if = "Option::is_none"` on both.
- `contrib/linux.conf`:
  - Fix `tcp_mem` link: `man8/tcp.7.html` → `https://man7.org/linux/man-pages/man7/tcp.7.html`
  - Normalize all `http://man7.org` links to `https://`
  - Update RFC 793 link to `https://datatracker.ietf.org/doc/html/rfc793`
  - Fix `dmesg`: add description
  - Fix `vmstat`: description says "memory usage" — replace with accurate multi-domain description
  - Fix `free`: thin description — add `-m` means MiB, mention buffers/page cache
  - Fix `slabinfo`: mentions only permissions — describe what it actually shows
  - Fix `socket_stat`: resolve "cf. man1" and "FRAG: currently unclear"
  - Populate `what_to_look_for` for: `vmstat`, `free`, `iostat`, `sar_cpu`, `sar_q`, `ss_all_tcp`,
    `socket_stat`, `tcp_mem`
  - Populate `install_hint` for commands whose binaries come from optional packages: all `sar_*`
    variants → `apt-get install sysstat`, `pidstat` → `apt-get install sysstat`
- `contrib/osx.conf`:
  - Fix `vm_stat` link: `1m/vmstat` → `https://www.unix.com/man-page/osx/1/vm_stat/`
  - Fix `vm_stat` description: add Apple Silicon 16 KB page size note
  - Populate `what_to_look_for` for: `vm_stat`, `iostat`, `memory_pressure`, `netstat_an`,
    `netstat_i`, `nettop_snapshot`

Phase complete when:
- `cargo test --all-features` passes
- `cargo check --all-features` passes
- `usereport --show-config` serializes the new fields without error
- Both config files parse without error (covered by existing config validation test)

### Test Scenarios

GIVEN `linux.conf` is loaded  
WHEN `Config::from_str` parses it  
THEN no error is returned and all commands with `install_hint` have non-empty values

GIVEN a `[[command]]` entry with `what_to_look_for = "look for X"`  
WHEN the config is serialized back with `toml::to_string_pretty`  
THEN the field round-trips correctly

GIVEN `linux.conf`  
WHEN all `[[command.links]]` URLs are inspected  
THEN none contain `http://` (all are `https://`) and `tcp_mem` links to `man7/tcp.7.html`

---

## Phase 2 — Report Completeness

Surface what was skipped, what was healthy, and add threshold context to the Signals table. Users must
be able to trust the completeness signal of the report.

**Changes:**

- `cli/config.rs`: expose `install_hint()` accessor on `Command`.
- `command.rs` / `CommandResult`: no structural change; `SkippedMissing { command, binary }` already
  carries the needed data.
- `analysis.rs`: no change to `AnalysisReport` — `checked_ok: Vec<String>` already exists.
- `rule/mod.rs`: after evaluating all rules, collect signal IDs that were tested by at least one rule
  predicate and for which no rule fired. Populate `AnalysisReport.checked_ok` with these IDs.
  - Requires the rule engine to return both findings AND the set of evaluated-but-passing signal IDs.
  - Change `RuleEngine::run` signature to return `(Vec<Finding>, Vec<String>)` where the second element
    is the checked-ok list.
- `contrib/html.j2` and `contrib/markdown.j2`:
  - Add a **Coverage Gaps** section rendered before Findings, iterating `command_results` for
    `SkippedMissing` variants. Show binary name, command name, and `install_hint` when present.
    Include "findings may be incomplete" notice when the list is non-empty.
  - Add a **Healthy** section rendered after Findings when `checked_ok` is non-empty.
  - Add a "vs. threshold" column to the Signals table. Build a threshold map in the template from
    rules context — or, simpler: pass a pre-computed `signal_thresholds: HashMap<String, ThresholdInfo>`
    into the template context from Rust (see Architecture note below).

**Architecture note — threshold index:**  
Parse the rule TOML's `when` predicates at startup into a
`HashMap<signal_id, Vec<(Severity, Op, f64)>>`. Pass this as a new template context variable
`signal_thresholds`. Template logic: look up the current signal's ID, find the lowest (most severe)
threshold, and render "ok", ">20% warn", ">4 CRIT", etc. Keep the parsing simple: only handle the
common `signal_id op number` predicate form; compound predicates are skipped silently.

Phase complete when:
- A report generated with a command whose binary is absent shows a Coverage Gaps section
- A report with no issues shows a Healthy section listing signals checked OK
- The Signals table renders a "vs. threshold" column with correct labels for known signals

### Test Scenarios

GIVEN `sar_cpu` binary is absent from PATH  
WHEN `usereport` runs the default profile  
THEN the rendered report contains "Coverage Gaps" and lists `sar_cpu` with its binary name

GIVEN `sar_cpu` has `install_hint = "apt-get install sysstat"`  
WHEN the Coverage Gaps section is rendered  
THEN the install hint appears inline

GIVEN cpu.iowait_pct = 5.0 and the rule `cpu.iowait_elevated` fires at > 20  
WHEN `RuleEngine::run` completes  
THEN `checked_ok` contains `"cpu.iowait_pct"` because the rule evaluated but did not fire

GIVEN cpu.iowait_pct = 25.0 and the rule fires  
WHEN `RuleEngine::run` completes  
THEN `checked_ok` does NOT contain `"cpu.iowait_pct"` (the rule fired)

GIVEN signal `cpu.iowait_pct` with value 15.0 and threshold at 20  
WHEN the Signals table is rendered  
THEN the threshold column shows "15 / 20 warn" or equivalent

---

## Phase 3 — Diagnostic Thread

Connect findings to the command outputs that show them, and connect suggest items to outputs already in
the report. Close the gap between the rule engine's view of the system and the human-readable evidence.

**Changes:**

- `finding.rs`: add `source_commands: Vec<String>` to `Evidence`. Default: empty vec. Serde:
  `skip_serializing_if = "Vec::is_empty"`.
- `collector/mod.rs`: add `fn source_commands(&self) -> &[&str] { &[] }` default method to the
  `Collector` trait.
- Each collector in `collector/`: implement `source_commands()` returning the command names whose
  output shows the same data:
  - `CpuCollector` → `["sar_cpu", "mpstat", "iostat"]`
  - `MemoryCollector` → `["free", "vmstat"]`
  - `DiskCollector` → `["iostat", "df"]`
  - `NetworkCollector` → `["sar_dev", "sar_tcp", "sar_edev", "ss_all_tcp", "netstat_tcp_s"]`
  - `HostCollector` → `["uptime"]`
  - macOS: `CpuCollector` → `["vm_stat", "iostat"]`, `MemoryCollector` → `["vm_stat", "memory_pressure"]`
- `rule/mod.rs`: when building a `Finding`, for each `Evidence`, cross-reference its `signal_id` against
  the collector map to populate `source_commands`. The rule engine already knows which signals fired;
  it needs a `HashMap<signal_id, Vec<String>>` (signal → source commands) passed in from `generate_report`.
- `cli/mod.rs` (`generate_report`): build the signal→source-commands map from all registered collectors
  before running the analysis. Pass into the rule engine.
- `contrib/html.j2`:
  - For each finding's evidence entry, render links `<a href="#cmd-{name}">{name}</a>` for each
    source command.
  - For each command result section, add `id="cmd-{command.name}"` to the heading element.
  - After the Findings section, render a **Evidence Commands** sub-section: pull the command result
    sections for all commands referenced in any finding's evidence, render them inline with a badge
    "Evidence for: [finding-id]".
  - Wrap remaining (non-evidence) command sections in `<details><summary>Full output (N commands)</summary>`.
  - In the `suggest` rendering: compare each suggest string against `command.command` values in
    `command_results`. When a match is found, render "(see: [command-title ↓])" with an anchor link
    instead of the raw command string.
- `contrib/markdown.j2`:
  - Add `(see: command-name)` inline references after evidence signal IDs when `source_commands` is set.
  - In suggest rendering, append "(see above)" when the suggest string matches a run command.

Phase complete when:
- A finding with `source_commands = ["iostat"]` renders an anchor link to the iostat section in HTML
- The iostat section has the matching `id` attribute
- Evidence command sections appear before non-evidence sections in HTML
- Non-evidence commands are collapsed in HTML
- A suggest item matching a run command renders a cross-reference rather than a bare command string

### Test Scenarios

GIVEN `CpuCollector.source_commands()` returns `["sar_cpu"]`  
AND the rule `cpu.iowait_elevated` fires with evidence `cpu.iowait_pct`  
WHEN the rule engine builds the Finding  
THEN `finding.evidence[0].source_commands` contains `"sar_cpu"`

GIVEN a Finding with `evidence[0].source_commands = ["iostat"]`  
WHEN the HTML template renders the finding  
THEN the evidence line contains `href="#cmd-iostat"`

GIVEN `command_results` contains a result for `iostat`  
WHEN the HTML template renders the command section  
THEN the heading has `id="cmd-iostat"`

GIVEN `suggest = ["iostat -x 1 5"]` on a finding  
AND `command_results` contains a command whose `.command` field is `"iostat -x 1 5"`  
WHEN the template renders the suggest list  
THEN the output reads "see [iostat ↓]" not "run `iostat -x 1 5`"

---

## Phase 4 — At-a-Glance Overview

Add the vital signs header, USE coverage map, and profile follow-up recommendations. Everything needed
to understand the system state at a glance before reading a single command output.

**Changes:**

- `cli/config.rs`:
  - Add `use_dimension: Option<UseDimension>` to `Command`, where
    `UseDimension { resource: UseResource, aspect: UseAspect }`,
    `UseResource` ∈ `{Cpu, Memory, Disk, Network}`,
    `UseAspect` ∈ `{Utilization, Saturation, Errors}`.
    Both enums derive `Serialize, Deserialize` with lowercase string values.
  - Add `followup: Vec<ProfileFollowup>` to `Profile`, where
    `ProfileFollowup { finding: String, recommend: String, reason: String }`.
  - Populate `use_dimension` in `linux.conf` and `osx.conf` for all existing commands:
    - `sar_cpu`, `mpstat` → `{cpu, utilization}`
    - `sar_q`, `vmstat` → `{cpu, saturation}`
    - `pidstat` → `{cpu, utilization}`
    - `free` → `{memory, utilization}`
    - `swapon`, `swap_usage` → `{memory, saturation}`
    - `meminfo`, `slabinfo` → `{memory, utilization}`
    - `iostat` → `{disk, utilization}`
    - `df` → `{disk, utilization}`
    - `sar_dev`, `netstat_i`, `nettop_snapshot` → `{network, utilization}`
    - `sar_edev`, `netstat_an` → `{network, errors}`
    - `sar_tcp`, `ss_all_tcp`, `socket_stat`, `tcp_mem` → `{network, saturation}`
    - `vm_stat` (macOS) → `{memory, utilization}`
    - `memory_pressure` (macOS) → `{memory, saturation}`
    - `iostat` (macOS) → `{disk, utilization}`
  - Populate `followup` in `linux.conf` default profile:
    - `{finding: "cpu.iowait_elevated", recommend: "mem", reason: "iowait often driven by memory pressure"}`
    - `{finding: "mem.pressure", recommend: "mem", reason: "full memory inspection"}`
    - `{finding: "net.retransmit_elevated", recommend: "net", reason: "TCP retransmits warrant full network investigation"}`

- `analysis.rs` / `AnalysisReport`: add a `use_coverage: Vec<UseCoverageEntry>` field (computed in
  `generate_report`, not in `Analysis::run`), where `UseCoverageEntry { resource, aspect, covered: bool }`.

- `cli/mod.rs` (`generate_report`): after building the report, compute `use_coverage` by cross-referencing
  the commands that ran (i.e. had a non-`SkippedMissing` result) against their `use_dimension` annotations.
  Mark a combination covered if at least one non-skipped command has that `{resource, aspect}`.

- **Vital signs computation**: also in `generate_report`, build a `VitalSigns` struct with four resource
  lines, each selecting the most relevant signals from `report.signals()`:
  - `CpuLine { load_ratio: Option<f64>, iowait_pct: Option<f64>, steal_pct: Option<f64>, trend: Option<Trend>, severity: Option<Severity> }`
  - `MemLine { used_pct: Option<f64>, swap_used_pct: Option<f64>, severity: Option<Severity> }`
  - `DiskLine { max_util_pct: Option<f64>, max_await_ms: Option<f64>, severity: Option<Severity> }`
  - `NetLine { retrans_pct: Option<f64>, drops: Option<i64>, severity: Option<Severity> }`
  
  `severity` on each line = highest severity of any finding whose evidence signal belongs to that resource
  (determined by the signal ID prefix: `cpu.*`, `mem.*`, `disk.*`, `net.*`). Pass `VitalSigns` into the
  template context.

- `contrib/html.j2` and `contrib/markdown.j2`:
  - Render the Vital Signs block as the first thing after the `<h1>` / report title.
  - Render the USE Coverage table after the Vital Signs block.
  - Render the "Where to investigate next" section after Findings, populated from `profile.followup`
    entries that match fired finding IDs.
  - Render the diff tip at the very bottom of the report when `findings` is non-empty.

Phase complete when:
- A report with signals present renders a Vital Signs block with all four resource lines
- A report where network commands were all skipped shows `[not profiled]` on the Network line
- The USE Coverage table marks `{network, saturation}` as covered when `sar_tcp` ran successfully
- A default-profile report with `cpu.iowait_elevated` fired shows the `mem` follow-up recommendation

### Test Scenarios

GIVEN `cpu.iowait_pct = 23.4` is in signals and `cpu.iowait_elevated` finding is present  
WHEN the template renders the Vital Signs block  
THEN the CPU line shows "iowait 23%" and a "WARN" badge

GIVEN all network commands produced `SkippedMissing`  
WHEN `use_coverage` is computed  
THEN all `{network, *}` entries have `covered: false`  
AND the Network Vital Signs line shows "[not profiled]"

GIVEN `linux.conf` default profile loads and `cpu.iowait_elevated` fires  
WHEN the report is rendered  
THEN a "Where to investigate next" section appears with "usereport --profile mem"

GIVEN findings are non-empty  
WHEN the report is rendered  
THEN a diff tip appears at the bottom with the `usereport diff` command

---

## Phase 5 — Output-Derived Signals

Let command outputs feed the rule engine. Bridges the gap between the collector world (signals from
`/proc`) and the command world (formatted tool output). Enables rules over `dmesg` entries, specific
process names in `pidstat`, OOM counts, etc.

**Changes:**

- `cli/config.rs`: add `extract: Vec<CommandExtract>` to `Command`, where
  ```rust
  pub struct CommandExtract {
      pub pattern: String,        // regex with named capture group `val`
      pub signal_id: String,
      pub unit: String,           // parsed into Unit enum
      pub aggregate: Aggregate,   // Last | Max | Min | Avg | Count
  }
  ```
  Add serde `default` on the `extract` vec so existing configs continue to parse.

- `command.rs` or new `src/extract.rs`: implement `fn extract_signals(result: &CommandResult, extracts: &[CommandExtract]) -> Vec<Signal>`.
  - Compile each `pattern` as a regex (error on invalid regex — surface as config validation failure).
  - For `Count` aggregate: count matching lines, emit `Signal { value: SignalValue::I64(count), .. }`.
  - For other aggregates: collect all `val` capture group values from matching lines, parse as f64,
    apply `max/min/avg/last`. Emit nothing if no lines match (warn in debug log).
  - `Signal.at` = current time; `Signal.id` = `extract.signal_id`.

- `analysis.rs` (`Analysis::run`): after running commands and before running collectors, call
  `extract_signals` for each `CommandResult::Success` whose `Command` has non-empty `extract` entries.
  Append the resulting signals to the signal pool before passing to the rule engine.

- Config validation: validate `pattern` is a valid regex and contains a named group `val` (unless
  `aggregate = "Count"`). Return a descriptive error from `Config::validate`.

- Add entries to `linux.conf` as proof-of-concept:
  ```toml
  # in dmesg command
  [[command.extract]]
  pattern = 'Out of memory:'
  signal_id = "dmesg.oom_count"
  unit = "count"
  aggregate = "count"

  # in vmstat command  
  [[command.extract]]
  pattern = '^\s*(?:\d+\s+){9}\d+\s+(?P<val>\d+)'
  signal_id = "vmstat.wa_pct"
  unit = "pct"
  aggregate = "max"
  ```

- Extend `usereport explain` to accept command names (in addition to rule/signal IDs):
  look up the command in config by name, render its `title`, `description`, `what_to_look_for`,
  `extract` entries, and `links`. Fall back to the existing rule/signal lookup if not found as a command.

Phase complete when:
- `dmesg.oom_count` appears in the signals list when `dmesg` runs and its output contains OOM lines
- An invalid regex in `[[command.extract]]` causes `Config::validate` to return an error
- `usereport explain vmstat` renders the command's metadata and `what_to_look_for`
- Extracted signals trigger rules — if `vmstat.wa_pct > 20` rule exists, it fires on extracted signal

### Test Scenarios

GIVEN a `CommandResult::Success` with stdout `"Out of memory: Killed process 123"` (2 lines)  
AND `extract = [{pattern: "Out of memory:", signal_id: "dmesg.oom_count", unit: "count", aggregate: "count"}]`  
WHEN `extract_signals` is called  
THEN it returns one `Signal` with `id = "dmesg.oom_count"` and `value = SignalValue::I64(1)`

GIVEN `aggregate = "max"` and three matching lines with `val` = 5, 18, 12  
WHEN `extract_signals` is called  
THEN the signal value is `18`

GIVEN `aggregate = "count"` (no `val` group required)  
AND the pattern matches 3 lines  
WHEN `extract_signals` is called  
THEN the signal value is `3`

GIVEN a command with `extract.pattern = "["` (invalid regex)  
WHEN `Config::validate` runs  
THEN it returns an error describing which command and which pattern is invalid

GIVEN `dmesg.oom_count` signal is extracted with value 2  
AND a rule `when = "dmesg.oom_count > 0"` with severity Crit exists  
WHEN the rule engine runs  
THEN a finding fires for `dmesg.oom_count`

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
│  Analysis::run                                                       │
│  1. Run commands (parallel) → Vec<CommandResult>                     │
│  2. extract_signals() over SkippedMissing-free results  (Phase 5)   │
│  3. Collectors → signals                                             │
│  4. Merge extracted + collector signals                              │
│  5. Baseline annotation                                              │
│  6. RuleEngine::run(signals) → (findings, checked_ok)   (Phase 2)   │
│  7. PatternEngine::run                                               │
│  8. sort_findings                                                    │
└──────────────────────────────────────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│  generate_report (cli/mod.rs)                                        │
│  Compute: signal_thresholds, use_coverage, VitalSigns   (Ph 2, 4)   │
│  Build: signal→source-commands map               (Phase 3)          │
└──────────────────────────────────────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────────────────┐
│  Template (minijinja)                                                │
│  Vital Signs → USE Coverage → Coverage Gaps → Findings (annotated)  │
│  Evidence commands (promoted) → Full output (collapsed)             │
│  Healthy → Where next → Diff tip                                    │
└──────────────────────────────────────────────────────────────────────┘
```

---

## File & Module Structure

| File | Change |
|------|--------|
| `src/cli/config.rs` | Add `install_hint`, `what_to_look_for`, `use_dimension`, `extract`, `followup` fields; add `UseDimension`, `CommandExtract`, `ProfileFollowup`, `Aggregate` types |
| `src/collector/mod.rs` | Add `source_commands()` default method to `Collector` trait |
| `src/collector/cpu.rs` | Implement `source_commands()` |
| `src/collector/memory.rs` | Implement `source_commands()` |
| `src/collector/disk.rs` | Implement `source_commands()` |
| `src/collector/network.rs` | Implement `source_commands()` |
| `src/collector/host.rs` | Implement `source_commands()` |
| `src/finding.rs` | Add `source_commands: Vec<String>` to `Evidence` |
| `src/rule/mod.rs` | Return `(Vec<Finding>, Vec<String>)` from `run`; populate `Evidence.source_commands` |
| `src/analysis.rs` | Pass checked_ok to report; accept source-command map |
| `src/extract.rs` | New: `extract_signals(result, extracts) -> Vec<Signal>` |
| `src/cli/mod.rs` | Compute `signal_thresholds`, `use_coverage`, `VitalSigns`; extend `run_explain` for commands |
| `contrib/linux.conf` | Fix links, descriptions, add `what_to_look_for`, `install_hint`, `use_dimension`, `extract`, `followup` |
| `contrib/osx.conf` | Fix links, descriptions, add `what_to_look_for`, `use_dimension` |
| `contrib/html.j2` | Add Vital Signs, USE Coverage, Coverage Gaps, threshold column, evidence promotion, collapse, cross-links, diff tip |
| `contrib/markdown.j2` | Add Vital Signs, USE Coverage, Coverage Gaps, threshold column, source-command refs, diff tip |

---

## Data Models

```rust
// cli/config.rs additions

pub struct UseDimension {
    pub resource: UseResource,
    pub aspect: UseAspect,
}

pub enum UseResource { Cpu, Memory, Disk, Network }
pub enum UseAspect  { Utilization, Saturation, Errors }

pub struct CommandExtract {
    pub pattern: String,
    pub signal_id: String,
    pub unit: String,
    pub aggregate: Aggregate,
}

pub enum Aggregate { Last, Max, Min, Avg, Count }

pub struct ProfileFollowup {
    pub finding: String,
    pub recommend: String,
    pub reason: String,
}

// finding.rs addition
pub struct Evidence {
    pub signal_id: String,
    pub observed: SignalValue,
    pub source_commands: Vec<String>,   // new
}

// Template context additions (cli/mod.rs)
pub struct ThresholdInfo {
    pub severity: Severity,
    pub op: String,      // ">" | ">=" | "<" | "<="
    pub value: f64,
}

pub struct VitalSigns {
    pub cpu: CpuLine,
    pub memory: MemLine,
    pub disk: DiskLine,
    pub network: NetLine,
}

pub struct UseCoverageEntry {
    pub resource: UseResource,
    pub aspect: UseAspect,
    pub covered: bool,
}
```

---

## Decision Log

| Decision | Alternatives considered | Reason chosen |
|----------|------------------------|---------------|
| `what_to_look_for` as a config field rather than a separate knowledge base | Separate `contrib/knowledge/*.toml` files | Config field is simpler, co-located with the command it describes, no extra loading logic. Knowledge base adds indirection without benefit at this scale. |
| Signal provenance via `source_commands` on `Evidence` rather than on `Finding` | On `Finding` directly | Evidence is the unit that carries signal data; provenance is per-signal, not per-finding. Multiple evidence entries in one finding may point to different commands. |
| `[[command.extract]]` with named group `val` | Column-index extraction; multiple named groups | Named group is readable and self-documenting. Multiple groups add complexity without a current use case (YAGNI). |
| Threshold index built from rule TOML predicates in Rust, passed to template | Compute in Jinja2 template | Simple predicates only (`signal op number`); computing in Rust avoids duplicating regex/parsing in template. Compound predicates silently skipped. |
| Vital Signs computed in Rust (`generate_report`) and passed as a struct | Pure Jinja2 template logic | Template logic for extracting the right signals per resource and finding the right severity would be complex and hard to test. Rust struct is testable and explicit. |
| Severity-gated layout uses HTML `<details>` for collapse | JavaScript-based toggle; always expanded | `<details>` requires no JS, works everywhere, and degrades gracefully. Markdown output stays fully expanded (no equivalent construct). |
| `followup` on `[[profile]]` rather than on `[[rule]]` | On rule (fire → recommend profile) | Profile authors know their domain — they can specify what to investigate next better than rule authors. Also avoids coupling rule files to profile names. |

---

## Open Decisions

**1. VitalSigns signal selection strategy**  
How to map signal IDs to resource lines: prefix matching (`cpu.*` → CPU line) vs. explicit annotation
on `Signal` (`resource: Option<UseResource>`).  
- **Prefix matching**: zero config, works immediately, fragile if signal IDs change  
- **Explicit annotation**: future-proof, requires changes to all collectors  
Impact: prefix matching is sufficient for Phase 4; annotation can be added later without breaking anything.  
**Tentative: prefix matching for Phase 4.**

**2. `explain` command — scope for Phase 5**  
Should `usereport explain <command-name>` also show which rules use signals extracted from that command?  
Requires cross-referencing `command.extract[].signal_id` against rule `evidence` fields.  
Impact: useful but adds complexity to `run_explain`; can be deferred to a follow-up.  
**Tentative: omit from Phase 5, add in a separate enhancement.**

---

## Out of Scope

- Line-level pattern highlighting in HTML (regex → colored lines): useful but orthogonal; can be
  added after Phase 4 without touching this design.
- Per-command sparklines (signal history over baseline records): requires baseline UI changes beyond
  what is designed here.
- Interactive TUI mode: entirely separate rendering path.
- Automatic `diff` comparison between two consecutive runs: separate UX flow.
- Workload-specific `what_to_look_for` content (postgres, java, nginx): content work, not
  architecture work; add to config files independently.
- macOS collector `source_commands` mappings: same design as Linux, straightforward to add but left
  to the implementer to verify command names match `osx.conf`.
