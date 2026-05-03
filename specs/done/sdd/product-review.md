# SDD: Fix Product Review Gaps

**Status**: Ready for Implementation
**Original**: specs/sdd/product-review.md
**Refined**: 2026-05-02

---

## Overview

A product review identified a set of critical and high-severity gaps between the README-documented behaviour and the actual shipped binary. The most serious are three fully-implemented subsystems that are never wired into the CLI entry point (pattern correlator, user rules loader, named baseline capture). This document describes the phased remediation plan across all severity levels found.

---

## Context & Constraints

- **Stack**: Rust 2024 edition, rust-version 1.85, single-crate workspace. All binary features gated behind `--all-features`. Entry point: `src/cli/mod.rs` (`generate_report`, `run_baseline`, `run_explain`, `run_diff`).
- **Key conventions** (CLAUDE.md): `cargo build/test/check --all-features`; surgical changes only; conventional commits; no `#[allow(...)]` without explanation.
- **Test strategy**: integration tests under `tests/` named `sdd_product_review_p<N>_c<N>_<desc>.rs`. Unit tests inline in source modules. Write failing tests before implementing fixes. Subprocess tests use `std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))` — no additional test crate is needed.
- **Integration test prefix**: `product_review` — all test files for this SDD use this prefix.
- **Phase gate**: each phase is complete when all its test scenarios pass under `cargo test --all-features`.
- **`SCHEMA_VERSION` note**: `raw_excerpts` element type changes from `String` to `LlmExcerpt`. Because `raw_excerpts` was always serialised as `[]` in all prior releases, no live consumer has parsed non-empty excerpt elements. This makes the change safe without a schema-version bump. `SCHEMA_VERSION` stays at `"1"`.

---

## Requirements

1. The system shall call `PatternEngine` (loaded from embedded pattern TOML strings) inside `generate_report` so that multi-signal patterns produce findings.
2. The system shall call `RulesLoader::new().with_builtins(builtin_rules()).with_user_dir(user_rules_dir).load()` inside `generate_report` and merge the loaded rules with any workload rules before constructing `RuleEngine`.
3. The system shall record actual collected signals (not an empty slice) when `usereport baseline record --name <N>` is invoked.
4. The system shall populate `LlmOutput.raw_excerpts` with truncated stdout excerpts from command results.
5. The system shall use the `OutputType` enum for the `diff` subcommand `--output` flag, with clap rejecting invalid values automatically.
6. The system shall look up signal IDs (extracted signal IDs from config commands, in addition to rule IDs and command names) in `usereport explain`.
7. The system shall render `CommandExtract` signal details in `explain` using human-readable strings from `Display` impls for `Aggregate` and `Unit` rather than Rust Debug format.
8. The system shall include the invalid profile name in "no such profile" config errors.
9. The system shall include both the profile name and command name in "profile command not found" and "hostinfo command not found" config errors.
10. The system shall include the filenames from the command line in `diff` text output headings ("Signals only in <fileA>" rather than "Signals only in A").
11. The system shall emit "No baselines recorded." when `usereport baseline list` finds no stored baselines.
12. The system shall list valid output format names when the user provides an unrecognised `--output` value for the root command.
13. The system shall escape `s.reason` with `| e` in the HTML template's hostinfo error section.
14. The README shall accurately document: the `--exit-on crit` exit code (2, not 1), the library API `Analysis::new` signature, that `install_hint` appears in `explain` not `check`, and that rolling baseline appends unconditionally.
15. The system shall add `max_parallel_commands`, `repetitions`, and `baseline_rolling_n` with commented examples to both `contrib/linux.conf` and `contrib/osx.conf`.
16. The system shall fix `"{n} binary/binaries not found"` to use singular/plural correctly.
17. The system shall print a warning to stderr when `--redact` is supplied without `--output llm`.

---

## File & Module Structure

Files touched by this SDD (no new files required):

```
src/cli/mod.rs            — Phase 1: pattern + rules wiring, baseline record, explain signal lookup,
                            explain unknown-topic error path, redact warning, plural fix
src/cli/config.rs         — Phase 4: error variants updated to carry profile/command name context
                            Phase 3: Display impl for Aggregate
src/signal.rs             — Phase 3: Display impl for Unit
src/llm.rs                — Phase 2: LlmExcerpt type, raw_excerpts population, redact parameter
src/diff.rs               — Phase 2 + Phase 4: output field type change; render_text takes filename labels
src/pattern/mod.rs        — Phase 1: add PatternEngine::empty() and extend_from() methods
contrib/html.j2           — Phase 5: XSS fix
contrib/linux.conf        — Phase 5: commented config keys
contrib/osx.conf          — Phase 5: commented config keys
README.md                 — Phase 5: documentation corrections
tests/sdd_product_review_p1_c1_pattern_engine_wired.rs
tests/sdd_product_review_p1_c2_user_rules_loaded.rs
tests/sdd_product_review_p1_c3_baseline_record_signals.rs
tests/sdd_product_review_p2_c1_raw_excerpts_populated.rs
tests/sdd_product_review_p2_c2_raw_excerpts_redacted.rs
tests/sdd_product_review_p2_c3_diff_output_typo_rejected.rs
tests/sdd_product_review_p2_c4_root_output_xyz_excludes_text.rs
tests/sdd_product_review_p3_c1_signal_id_resolved.rs
tests/sdd_product_review_p3_c2_extract_display_human_readable.rs
tests/sdd_product_review_p3_c3_unknown_topic_error.rs
tests/sdd_product_review_p4_c1_profile_name_in_error.rs
tests/sdd_product_review_p4_c2_diff_shows_filenames.rs
tests/sdd_product_review_p4_c3_baseline_list_empty_state.rs
tests/sdd_product_review_p4_c4_output_invalid_lists_values.rs
tests/sdd_product_review_p4_c5_binary_plural.rs
tests/sdd_product_review_p4_c6_redact_warning.rs
tests/sdd_product_review_p5_c1_html_xss_fix.rs
```

The `contrib/patterns/` directory already exists and contains six TOML files:
`lock_contention.toml`, `nfs_stall.toml`, `slab_leak.toml`, `socket_leak.toml`, `thundering_herd.toml`, `time_wait.toml`.

---

## Data Models

### LlmOutput changes (`src/llm.rs`)

```rust
pub struct LlmOutput {
    // existing fields unchanged
    pub raw_excerpts: Vec<LlmExcerpt>,  // type changes from Vec<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExcerpt {
    pub command: String,
    pub output: String,
}

impl LlmOutput {
    /// Compile-time constant for excerpt truncation.
    pub const MAX_EXCERPT_CHARS: usize = 1_000;

    /// Signature change: add `redact: bool` parameter.
    pub fn from_report(report: &AnalysisReport, redact: bool) -> Self { ... }
}
```

### Config error variants (`src/cli/config.rs`)

`InvalidConfig { reason: &'static str }` is removed entirely. Replace all three call sites with typed variants:

```rust
pub enum Error {
    // existing variants unchanged:
    ParseConfigFailed { source: toml::de::Error },
    ReadConfigFileFailed { path: PathBuf, source: std::io::Error },
    InvalidExtractPattern { command: String, pattern: String, reason: String },
    // replaces InvalidConfig for the "no such profile" case:
    NoSuchProfile { name: String },
    // replaces InvalidConfig for the "profile command not found" case:
    ProfileCommandNotFound { profile: String, command: String },
    // replaces InvalidConfig for the "hostinfo command not found" case:
    HostinfoCommandNotFound { command: String },
}
```

Display strings (for `thiserror`):
- `NoSuchProfile` → `"no such profile '{name}'"`
- `ProfileCommandNotFound` → `"profile '{profile}': command '{command}' not found"`
- `HostinfoCommandNotFound` → `"hostinfo: command '{command}' not found"`

The three call sites in `src/cli/config.rs` are `Config::profile()` (line with `Error::InvalidConfig { reason: "no such profile" }`), `validate_host_info()`, and `validate_profiles_commands()`.

### Display impls for Aggregate and Unit

Add `impl fmt::Display for Aggregate` in `src/cli/config.rs`:
- `Count` → `"count"`, `Last` → `"last"`, `Max` → `"max"`, `Min` → `"min"`, `Avg` → `"avg"`

Add `impl fmt::Display for Unit` in `src/signal.rs`:
- `Pct` → `"percent"`, `MillisPerOp` → `"ms"`, `BytesPerSec` → `"bytes/s"`, `Count` → `"count"`, `Iops` → `"iops"`, `Microseconds` → `"µs"`, `Hz` → `"hz"`, `Celsius` → `"celsius"`, `None` → `"none"`

### PatternEngine helpers (`src/pattern/mod.rs`)

The `patterns` field of `PatternEngine` is private. Add two methods:

```rust
impl PatternEngine {
    pub fn empty() -> Self {
        Self { patterns: vec![] }
    }

    pub fn extend_from(&mut self, other: PatternEngine) {
        self.patterns.extend(other.patterns);
    }
}
```

Unit-test both methods inline in `src/pattern/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_patterns() {
        let pe = PatternEngine::empty();
        // PatternEngine has no public len(); use extend_from to verify nothing is merged
        let mut other = PatternEngine::empty();
        other.extend_from(pe);
        // If both are empty, the merged engine should also be empty (no panic)
    }

    #[test]
    fn extend_from_merges_patterns() {
        // Build a minimal valid TOML pattern and verify the merged engine
        // has twice the patterns when extended from itself (via clone if available,
        // or from_toml twice).
        let toml = r#"
[[pattern]]
id = "test.p1"
description = "test"
severity = "Warn"
[[pattern.conditions]]
signal = "cpu.idle_pct"
op = "lt"
threshold = 10.0
"#;
        let pe1 = PatternEngine::from_toml(toml).unwrap();
        let pe2 = PatternEngine::from_toml(toml).unwrap();
        let mut merged = PatternEngine::empty();
        merged.extend_from(pe1);
        merged.extend_from(pe2);
        // Merged engine should not panic when used; content verified by integration tests
    }
}
```

### Pattern loading in `generate_report` (`src/cli/mod.rs`)

In the `defaults` module (alongside existing `CONFIG`, `HTML_TEMPLATE`, etc.), add:

```rust
pub(crate) static PATTERNS: &[&str] = &[
    include_str!("../../contrib/patterns/lock_contention.toml"),
    include_str!("../../contrib/patterns/nfs_stall.toml"),
    include_str!("../../contrib/patterns/slab_leak.toml"),
    include_str!("../../contrib/patterns/socket_leak.toml"),
    include_str!("../../contrib/patterns/thundering_herd.toml"),
    include_str!("../../contrib/patterns/time_wait.toml"),
];
```

### OutputType::Text variant (`src/cli/mod.rs`)

Add `Text` to `OutputType`:

```rust
pub enum OutputType {
    Template,
    Html,
    Json,
    Markdown,
    Llm,
    Text,  // new variant, used only by diff subcommand
}
```

Add to `impl clap::ValueEnum for OutputType`:
- Include `OutputType::Text` in `value_variants()`.
- Map `OutputType::Text => clap::builder::PossibleValue::new("text")`.

Add to `impl FromStr for OutputType`:
- `"text" => Ok(OutputType::Text)`.

The `FromStr` error message (root `--output`) must not include `text` in valid values since `text` is only valid for `diff`. Valid values string: `"template, html, json, markdown, llm"`.

In `generate_report`, the match on `opt.output` must include a catch-all or explicit arm for `OutputType::Text` that returns an error:
```rust
OutputType::Text => {
    return Err(miette!("output type 'text' is only valid for the diff subcommand; valid values: template, html, json, markdown, llm"));
}
```

### diff subcommand output field

Current `Subcommand::Diff` uses `output: String` with `default_value = "text"`. Change to:

```rust
Diff {
    a: PathBuf,
    b: PathBuf,
    #[arg(long, default_value = "text", value_enum)]
    output: OutputType,
}
```

Change `run_diff` signature:
```rust
fn run_diff(a_path: &PathBuf, b_path: &PathBuf, output: &OutputType) -> miette::Result<()>
```

Match arm in `run_diff`:
- `OutputType::Text` maps to `diff::render_text(...)`.
- `OutputType::Json` maps to `serde_json::to_writer_pretty(std::io::stdout(), &diff_report).into_diagnostic().context("write diff JSON")?;`
- All other arms: `return Err(miette!("output type not supported for diff; valid values: text, json"))`.

### diff render_text labels

Change `diff::render_text` signature to accept label strings:

```rust
pub fn render_text<W: std::io::Write>(
    d: &DiffReport,
    label_a: &str,
    label_b: &str,
    mut w: W,
) -> std::io::Result<()>
```

Callers pass `a_path.display().to_string()` and `b_path.display().to_string()`. Replace all four heading literals `"## Signals only in A"`, `"## Signals only in B"`, `"## Signals changed"` etc. with `format!("## ... in {label_a}")` / `format!("## ... in {label_b}")`.

---

## API Contracts

### `LlmOutput::from_report` (updated)

```
Input:  &AnalysisReport, redact: bool
Output: LlmOutput
Side effects: none
```

`raw_excerpts` is populated from `report.command_results().first().map(|v| v.as_slice()).unwrap_or(&[])`. For each `CommandResult::Success { command, stdout, .. }` entry, append `LlmExcerpt { command: command.name().to_string(), output: stdout.chars().take(MAX_EXCERPT_CHARS).collect() }`. If `redact` is true, apply `Redactor::from_env().redact_output(llm_out)` before returning.

Remove the existing per-caller redaction blocks (they currently appear in `run_convert` around line 699 and in `generate_report` around line 1061) and rely solely on the `redact` parameter in `from_report`.

Call site updates (use function names as anchors, not line numbers):
- In `run_convert`: `LlmOutput::from_report(&report, redact)` — remove the `if redact { ... }` block that follows.
- In `generate_report` (inside `if opt.output == OutputType::Llm`): `LlmOutput::from_report(&report, opt.redact)` — remove the `if opt.redact { ... }` block that follows.

### `PatternEngine::empty` / `extend_from`

```
PatternEngine::empty() -> PatternEngine         // returns empty engine
extend_from(&mut self, other: PatternEngine)    // merges other.patterns into self
```

---

## Configuration

No new config keys. Phase 5 adds **commented-out** example lines to the `[defaults]` section of both `contrib/linux.conf` and `contrib/osx.conf`:

```toml
# max_parallel_commands = 8
# repetitions = 1
# baseline_rolling_n = 10
```

These lines must appear in `[defaults]` after the existing uncommented keys.

---

## Error Handling

| Failure | Trigger | Behaviour | User-visible |
|---|---|---|---|
| Malformed pattern TOML in `defaults::PATTERNS` | Embedded file is invalid TOML | `log::warn!` and skip; `generate_report` continues | `"pattern: failed to parse embedded pattern, skipping: {err}"` |
| `--output xyz` on root command | `FromStr` for `OutputType` returns `Err` | clap exits non-zero | `"failed to parse xyz as output type; valid values: template, html, json, markdown, llm"` |
| `--output text` on root command | `generate_report` match arm | Returns `Err(miette!(...))`, exits non-zero | `"output type 'text' is only valid for the diff subcommand; valid values: template, html, json, markdown, llm"` |
| `--output jsn` on `diff` subcommand | clap `value_enum` rejects unknown variant | clap exits non-zero with auto-generated error | `"error: invalid value 'jsn' for '--output <OUTPUT>'\n  [possible values: template, html, json, markdown, llm, text]"` |
| No such profile | `Config::profile()` called with unknown name | Return `Error::NoSuchProfile { name }` | `"no such profile 'nonexistent'"` |
| Profile command not found | `validate_profiles_commands` detects missing command | Return `Error::ProfileCommandNotFound { profile, command }` | `"profile 'p': command 'c' not found"` |
| Hostinfo command not found | `validate_host_info` detects missing command | Return `Error::HostinfoCommandNotFound { command }` | `"hostinfo: command 'c' not found"` |
| `explain` unknown topic | ID matches no command, rule, or signal | Return `Err(miette!(...))` | `"unknown topic 'x'\n\nKnown topics:\n  ..."` |
| `--redact` without `--output llm` | `opt.redact && opt.output != OutputType::Llm` | Print warning to stderr, continue | `"warning: --redact has no effect unless --output llm is also set"` |
| `baseline list` returns empty | `store.list()` returns `Ok(vec![])` | Print message, exit 0 | `"No baselines recorded."` |

---

## Implementation Phases

## Phase 1 — Wire pattern engine, user rules, and baseline record

Three fully-built subsystems are never called from the CLI. Fix all three in this phase, each as a separate commit, all touching `src/cli/mod.rs` and `src/pattern/mod.rs`.

**1a — PatternEngine::empty() and extend_from() (`src/pattern/mod.rs`)**

Add to `impl PatternEngine`:

```rust
pub fn empty() -> Self {
    Self { patterns: vec![] }
}

pub fn extend_from(&mut self, other: PatternEngine) {
    self.patterns.extend(other.patterns);
}
```

Also add the inline unit tests from the Data Models section. Phase 1a has no independent integration test file; its correctness is verified through Phase 1b. The unit tests in `src/pattern/mod.rs` serve as the Phase 1a gate.

**1b — Pattern engine wiring (`generate_report` in `src/cli/mod.rs`)**

1. Add `PATTERNS` static to the `defaults` module (six `include_str!` entries for all files in `contrib/patterns/`; list all six explicitly as shown in Data Models).
2. After the closing `}` of the `if let Some(cgroup_path)` block (which sets `analysis = analysis.with_cgroup(cgroup_path);`) and before `let mut report = analysis.run(context)`, insert:

```rust
let mut pattern_engine = PatternEngine::empty();
for text in defaults::PATTERNS {
    match PatternEngine::from_toml(text) {
        Ok(pe) => pattern_engine.extend_from(pe),
        Err(e) => log::warn!("pattern: failed to parse embedded pattern, skipping: {e}"),
    }
}
analysis = analysis.with_pattern_engine(pattern_engine);
```

**1c — User rules wiring (`generate_report` in `src/cli/mod.rs`)**

Replace `let mut all_rules = builtin_rules();` with:

```rust
let user_rules_dir = std::env::var("XDG_CONFIG_HOME")
    .map(PathBuf::from)
    .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
    .ok()
    .map(|d| d.join("usereport").join("rules.d"));

let mut loader = RulesLoader::new().with_builtins(builtin_rules());
if let Some(dir) = user_rules_dir {
    loader = loader.with_user_dir(dir);
}
let rules_result = loader.load();
let mut all_rules = rules_result.rules;
```

After `analysis.run(context)` and the `drop(analysis)` call, push load findings into the report:

```rust
for finding in rules_result.load_findings {
    report.findings.push(finding);
}
```

Add `RulesLoader` and `RulesLoadResult` to the import from `crate::rule` (currently `rule::{Rule, RuleEngine, builtin::builtin_rules}`).

**1d — Baseline record signals (`run_baseline` in `src/cli/mod.rs`)**

In the `BaselineAction::Record { name }` arm, replace `store.record(label, &[])` with a real collection pass. Extract a private helper:

```rust
// Only HostCollector and CpuCollector are used here: they are guaranteed to return
// at least one signal on any host, and baseline record is a lightweight snapshot.
// Using all eight collectors would pull in disk/network/etc. without a matching
// improvement in baseline fidelity.
fn collect_signals_for_baseline() -> Vec<Signal> {
    use crate::collector::{Collector, CollectCtx};
    let ctx = CollectCtx { cgroup_path: None };
    let collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(HostCollector::new()),
        Box::new(CpuCollector::new()),
    ];
    let mut signals = Vec::new();
    for mut c in collectors {
        match c.collect(&ctx) {
            Ok(mut s) => signals.append(&mut s),
            Err(e) => log::warn!("baseline collect: {e}"),
        }
    }
    signals
}
```

Note: use `CollectCtx { cgroup_path: None }` (struct literal). Do not use `CollectCtx::default()` unless `#[derive(Default)]` is confirmed on the struct; the struct literal is always safe. If `CollectCtx` has additional fields beyond `cgroup_path`, set them to their zero values or verify the struct definition in `src/collector/mod.rs` first and adjust accordingly.

Then in `BaselineAction::Record`:

```rust
let signals = collect_signals_for_baseline();
store.record(label, &signals)
    .into_diagnostic()
    .with_context(|| format!("record baseline '{}'", label))?;
```

`HostCollector` and `CpuCollector` are already imported at the top of `src/cli/mod.rs`. `CollectCtx` must be added to the collector import. `Signal` must be in scope (already imported via `crate::signal::Signal` or the analysis import). The `use crate::collector::{Collector, CollectCtx}` inside the function body is local; add it there rather than at module level.

**Phase complete when**: `cargo test --all-features` passes for all Phase 1 test files, and the inline unit tests in `src/pattern/mod.rs` pass.

### Test Scenarios

- GIVEN the embedded `contrib/patterns/time_wait.toml` pattern whose predicate matches when `tcp.time_wait_count > 500` WHEN `generate_report` runs against a signal set satisfying that condition THEN `report.findings()` contains at least one entry whose `id` matches the pattern `id` field from `time_wait.toml`. *(file: `tests/sdd_product_review_p1_c1_pattern_engine_wired.rs`)*
- GIVEN `XDG_CONFIG_HOME` set to a temp directory containing `usereport/rules.d/test.toml` with rule `id = "test.user_rule"` and a predicate that always matches WHEN `usereport --output json` is invoked THEN the JSON output contains a finding with `rule_id == "test.user_rule"`. *(file: `tests/sdd_product_review_p1_c2_user_rules_loaded.rs`)*
- GIVEN the binary is invoked with no `--config` flag (uses builtin default config) WHEN `usereport baseline record --name smoke` is invoked THEN exit code is 0 AND the baseline file on disk contains a `signals` map with at least one key present (non-empty map; value may be zero). *(file: `tests/sdd_product_review_p1_c3_baseline_record_signals.rs`)*

---

## Phase 2 — LLM raw_excerpts and diff --output type safety

Two independent correctness gaps in separate files. Each sub-item is one commit.

**2a — `src/llm.rs` — raw_excerpts**

1. Add `pub struct LlmExcerpt { pub command: String, pub output: String }` (derive `Debug, Clone, Serialize, Deserialize`).
2. Change `raw_excerpts: Vec<String>` to `raw_excerpts: Vec<LlmExcerpt>`.
3. Change `from_report` signature to `pub fn from_report(report: &AnalysisReport, redact: bool) -> Self`.
4. Populate `raw_excerpts` from `report.command_results().first().map(|v| v.as_slice()).unwrap_or(&[])`. For each `CommandResult::Success { command, stdout, .. }`, append `LlmExcerpt { command: command.name().to_string(), output: stdout.chars().take(Self::MAX_EXCERPT_CHARS).collect() }`.
5. If `redact` is true, apply `Redactor::from_env().redact_output(llm_out)` before returning. Add `use crate::redact::Redactor;` import.
6. Update both call sites in `src/cli/mod.rs`:
   - In `run_convert` (function `run_convert`, inside `if *output == OutputType::Llm`): change `LlmOutput::from_report(&report)` to `LlmOutput::from_report(&report, *redact)`. Remove the `if redact { let redactor = Redactor::from_env(); llm_out = redactor.redact_output(llm_out); }` block that currently follows.
   - In `generate_report` (inside `if opt.output == OutputType::Llm`): change `LlmOutput::from_report(&report)` to `LlmOutput::from_report(&report, opt.redact)`. Remove the `if opt.redact { ... }` block that currently follows.

**2b — `src/cli/mod.rs` + `src/diff.rs` — diff --output type safety**

1. Add `OutputType::Text` variant and wire it into `clap::ValueEnum` and `FromStr` as specified in Data Models.
2. Change `Subcommand::Diff { output: String }` to `output: OutputType` with `#[arg(long, default_value = "text", value_enum)]`.
3. Add a match arm in `generate_report` for `OutputType::Text` that returns `Err(miette!("output type 'text' is only valid for the diff subcommand; valid values: template, html, json, markdown, llm"))`.
4. Change `run_diff` signature to `fn run_diff(a_path: &PathBuf, b_path: &PathBuf, output: &OutputType) -> miette::Result<()>`.
5. Update the match in `run_diff`:
   - `OutputType::Json => { serde_json::to_writer_pretty(std::io::stdout(), &diff_report).into_diagnostic().context("write diff JSON")?; }`
   - `OutputType::Text => { diff::render_text(...) }`
   - All other arms: `_ => return Err(miette!("output type not supported for diff; valid values: text, json"))`.
6. The `Subcommand::Diff` dispatch in `run_subcommand` already passes `output` by reference — no change needed there.
7. Update the `FromStr` error message: `Err(miette!("failed to parse {} as output type; valid values: template, html, json, markdown, llm", s))`. (`text` is intentionally excluded because it is only valid for `diff`.)

**Phase complete when**: `cargo test --all-features` passes for all Phase 2 test files.

### Test Scenarios

- GIVEN a JSON report where at least one `CommandResult::Success` entry has `stdout` longer than 1000 characters WHEN `usereport convert report.json --output llm` runs THEN `raw_excerpts` is a non-empty array AND every entry's `output` field is at most 1000 characters. *(file: `tests/sdd_product_review_p2_c1_raw_excerpts_populated.rs`)*
- GIVEN a JSON report where a command stdout contains `myhostname` and `USEREPORT_REDACT_SALT` is set WHEN `usereport convert report.json --output llm --redact` runs THEN no element of `raw_excerpts[*].output` contains the string `myhostname`. *(file: `tests/sdd_product_review_p2_c2_raw_excerpts_redacted.rs`)*
- GIVEN `usereport diff a.json b.json --output jsn` WHEN the command runs THEN exit code is non-zero AND stderr contains `jsn` AND at least one valid format name such as `json` or `text`. *(file: `tests/sdd_product_review_p2_c3_diff_output_typo_rejected.rs`)*
- GIVEN `usereport --output xyz` WHEN the command runs THEN exit code is non-zero AND stderr contains each of `template`, `html`, `json`, `markdown`, `llm` AND does not contain the word `text`. *(file: `tests/sdd_product_review_p2_c4_root_output_xyz_excludes_text.rs`)*

---

## Phase 3 — explain: signal ID support and Debug format fix

Two gaps in `run_explain` and `run_explain_command` in `src/cli/mod.rs`. Each sub-item is one commit.

Note: step 3d depends on `all_signal_ids` built in step 3c. Implement 3c before 3d.

**3a — Display impls for Aggregate and Unit**

Add `impl fmt::Display for Aggregate` in `src/cli/config.rs`:

```rust
impl fmt::Display for Aggregate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Aggregate::Count => "count",
            Aggregate::Last  => "last",
            Aggregate::Max   => "max",
            Aggregate::Min   => "min",
            Aggregate::Avg   => "avg",
        })
    }
}
```

Add `impl fmt::Display for Unit` in `src/signal.rs`:

```rust
impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Unit::Pct          => "percent",
            Unit::MillisPerOp  => "ms",
            Unit::BytesPerSec  => "bytes/s",
            Unit::Count        => "count",
            Unit::Iops         => "iops",
            Unit::Microseconds => "µs",
            Unit::Hz           => "hz",
            Unit::Celsius      => "celsius",
            Unit::None         => "none",
        })
    }
}
```

**3b — Debug format fix in `run_explain_command`**

In `run_explain_command`, change the extract `writeln!` from:

```rust
"Extract: {} ({:?} {:?}) pattern={}"
extract.signal_id, extract.aggregate, extract.unit, extract.pattern
```

to:

```rust
writeln!(out, "Signal ID: {}", extract.signal_id).into_diagnostic()?;
writeln!(out, "  Aggregate: {}", extract.aggregate).into_diagnostic()?;
writeln!(out, "  Unit:      {}", extract.unit).into_diagnostic()?;
writeln!(out, "  Pattern:   {}", extract.pattern).into_diagnostic()?;
```

**3c — Signal ID lookup in `run_explain`**

After the command-name lookup and before the rule-ID lookup, add:

```rust
// Collect all signal IDs from config extract definitions
let all_signal_ids: Vec<(&str, &crate::command::Command)> = config
    .commands
    .iter()
    .flat_map(|cmd| cmd.extract().iter().map(move |ex| (ex.signal_id.as_str(), cmd)))
    .collect();

// Use `_` for the signal ID since `id` (the parameter) is already in scope
if let Some((_, cmd)) = all_signal_ids.iter().find(|(sid, _)| *sid == id) {
    // Second scan is intentional: finds all commands that emit this signal ID,
    // not just the first one (which `cmd` points to).
    let emitting_commands: Vec<&str> = config
        .commands
        .iter()
        .filter(|c| c.extract().iter().any(|ex| ex.signal_id == id))
        .map(|c| c.name())
        .collect();
    writeln!(handle, "Signal ID: {id}").into_diagnostic()?;
    writeln!(handle, "Emitted by: {}", emitting_commands.join(", ")).into_diagnostic()?;
    // Print extract details from the first matching command
    for extract in cmd.extract().iter().filter(|ex| ex.signal_id == id) {
        writeln!(handle, "  Aggregate: {}", extract.aggregate).into_diagnostic()?;
        writeln!(handle, "  Unit:      {}", extract.unit).into_diagnostic()?;
        writeln!(handle, "  Pattern:   {}", extract.pattern).into_diagnostic()?;
    }
    return Ok(());
}
```

**3d — Unknown topic error path in `run_explain`** (depends on 3c)

Replace the `eprintln!` + `std::process::exit(1)` block (currently in the final `else` branch) with:

```rust
let mut known: Vec<String> = Vec::new();
for c in &config.commands {
    known.push(format!("  {} (command)", c.name()));
}
for r in &all_rules {
    known.push(format!("  {} (rule)", r.id));
}
for (sid, _) in &all_signal_ids {
    known.push(format!("  {} (signal)", sid));
}
known.sort();
return Err(miette!(
    "unknown topic '{}'\n\nKnown topics:\n{}",
    id,
    known.join("\n"),
));
```

The list is sorted alphabetically, newline-separated, uncapped.

**Phase complete when**: `cargo test --all-features` passes for all Phase 3 test files.

### Test Scenarios

- GIVEN a config where command `mem-check` has `[[command.extract]]` with `signal_id = "mem.free_pct"` WHEN `usereport explain mem.free_pct` runs THEN exit code is 0 AND stdout contains `mem.free_pct` AND stdout contains `mem-check`. *(file: `tests/sdd_product_review_p3_c1_signal_id_resolved.rs`)*
- GIVEN a config command with an extract entry where `aggregate = "last"` and `unit = "percent"` WHEN `usereport explain <that-command-name>` runs THEN stdout contains `last` AND contains `percent` AND does not contain `Last` surrounded by parentheses or braces AND does not contain `Percent`. *(file: `tests/sdd_product_review_p3_c2_extract_display_human_readable.rs`)*
- GIVEN `usereport explain totally_unknown_id` WHEN the command runs THEN exit code is non-zero AND error output contains `unknown topic` AND contains `totally_unknown_id`. *(file: `tests/sdd_product_review_p3_c3_unknown_topic_error.rs`)*

---

## Phase 4 — Error message quality

Six targeted fixes. Each sub-item is one commit. Steps 4e (grammar) and 4f (redact warning) are behaviour changes and belong here, not in Phase 5.

**4a — Config error context (`src/cli/config.rs`)**

Remove `InvalidConfig { reason: &'static str }` from `pub enum Error`. Add three typed variants as specified in Data Models. Update the three call sites:
- `Config::profile()`: use `Error::NoSuchProfile { name: profile_name.to_string() }`.
- `validate_host_info()`: use `Error::HostinfoCommandNotFound { command: c.clone() }`.
- `validate_profiles_commands()`: use `Error::ProfileCommandNotFound { profile: p.name.clone(), command: c.clone() }`.

Update all existing unit tests in `src/cli/config.rs` that assert on these error strings.

**4b — diff filenames (`src/diff.rs`)**

Change `render_text` signature as specified in Data Models. Replace the four heading literals with `format!` calls using `label_a` and `label_b`. Update the call in `run_diff` to pass `a_path.display().to_string()` and `b_path.display().to_string()`.

**4c — baseline list empty state (`run_baseline` in `src/cli/mod.rs`)**

Replace the entire `BaselineAction::List` arm body with:

```rust
let names = store.list().into_diagnostic().context("list baselines")?;
if names.is_empty() {
    println!("No baselines recorded.");
} else {
    for name in names {
        println!("{}", name);
    }
}
```

**4d — `--output` invalid value with valid names (`src/cli/mod.rs`)**

The `FromStr for OutputType` error path already exists. Change the error string to:

```rust
Err(miette!("failed to parse {} as output type; valid values: template, html, json, markdown, llm", s))
```

**4e — Grammar fix (`src/cli/mod.rs`)**

Replace:
```rust
eprintln!("{} binary/binaries not found", missing);
```
with:
```rust
eprintln!("{} {} not found", missing, if missing == 1 { "binary" } else { "binaries" });
```

**4f — `--redact` warning (`src/cli/mod.rs`)**

In `Opt::validate` (the method at `impl Opt`), after the `OutputType::Template` check, add:

```rust
if self.redact && self.output != OutputType::Llm {
    eprintln!("warning: --redact has no effect unless --output llm is also set");
}
```

The `Convert` subcommand also has `--redact`. In `run_convert`, add the same guard as the very first statement of the function, before any file I/O or early returns:

```rust
if redact && *output != OutputType::Llm {
    eprintln!("warning: --redact has no effect unless --output llm is also set");
}
```

**Phase complete when**: `cargo test --all-features` passes for all Phase 4 test files.

### Test Scenarios

- GIVEN a config that defines no profile named `nonexistent` WHEN `usereport --profile nonexistent` runs THEN exit code is non-zero AND stderr contains both `profile` and `nonexistent`. *(file: `tests/sdd_product_review_p4_c1_profile_name_in_error.rs`)*
- GIVEN two valid baseline JSON files `before.json` and `after.json` WHEN `usereport diff before.json after.json` produces text output THEN stdout contains the substring `before.json` AND contains `after.json` as part of section headings. *(file: `tests/sdd_product_review_p4_c2_diff_shows_filenames.rs`)*
- GIVEN no baseline files exist in the baseline store directory WHEN `usereport baseline list` runs THEN exit code is 0 AND stdout contains `No` and `baseline`. *(file: `tests/sdd_product_review_p4_c3_baseline_list_empty_state.rs`)*
- GIVEN `usereport --output xyz` WHEN the command runs THEN exit code is non-zero AND stderr contains at least one of `markdown`, `html`, `json`, `template`, `llm`. *(file: `tests/sdd_product_review_p4_c4_output_invalid_lists_values.rs`)*
- GIVEN one missing binary WHEN the not-found message is emitted THEN it reads `1 binary not found`. GIVEN two missing binaries THEN it reads `2 binaries not found`. *(file: `tests/sdd_product_review_p4_c5_binary_plural.rs`)*
- GIVEN `usereport --output markdown --redact` WHEN the command runs THEN stderr contains a warning referencing `--redact` and stating it has no effect without `--output llm`. *(file: `tests/sdd_product_review_p4_c6_redact_warning.rs`)*

---

## Phase 5 — Documentation fixes and minor polish

No behaviour-changing code changes except the HTML template XSS fix. README and config file edits. Each sub-item is one commit.

**5a — HTML XSS (`contrib/html.j2`)**

The following is the complete and exhaustive list of `{{ expr }}` expressions in `contrib/html.j2` that render user-supplied strings and are missing `| e`. Add `| e` to each one exactly as shown. Do not add `| e` to numeric values, boolean checks, or static template strings.

Expressions to fix (identified by surrounding context for unambiguous location):

1. **Page title and h1** — `{{ context.hostname }}` appears twice (line 7 and line 34). Change both to `{{ context.hostname | e }}`.
2. **Summary list — Kernel** — `{{ context.uname }}` (line 39). Change to `{{ context.uname | e }}`.
3. **Run Configuration — context.more items** — `{{ key }}` and `{{ value }}` (line 185). Change to `{{ key | e }}` and `{{ value | e }}`.
4. **Hostinfo Success — title/name in `<h3>`** (line 199): `{{ s.command.title }}` and `{{ s.command.name }}` inside `{% if s.command.title %}...{% else %}...{% endif %}`. Change to `{{ s.command.title | e }}` and `{{ s.command.name | e }}`.
5. **Hostinfo Success — stdout in `<pre>`** (line 203): `{{ s.stdout }}`. Change to `{{ s.stdout | e }}`.
6. **Hostinfo Success — command string** (line 209): `{{ s.command.command }}`. Change to `{{ s.command.command | e }}`. Also `{{ link.url }}` and `{{ link.name }}` on the same line. Change to `{{ link.url | e }}` and `{{ link.name | e }}`.
7. **Hostinfo Failed — title/name in `<h3>`** (line 216): same pattern as item 4. Change to `{{ s.command.title | e }}` / `{{ s.command.name | e }}`.
8. **Hostinfo Failed — stdout in `<pre>`** (line 219): `{{ s.stdout }}`. Change to `{{ s.stdout | e }}`.
9. **Hostinfo Failed — command string and links** (line 225): same pattern as item 6. Change to `{{ s.command.command | e }}`, `{{ link.url | e }}`, `{{ link.name | e }}`.
10. **Hostinfo Timeout — title/name in `<h3>`** (line 233): same pattern as item 4. Change to `{{ s.command.title | e }}` / `{{ s.command.name | e }}`.
11. **Hostinfo Timeout — command string** (line 236): `{{ s.command.command }}`. Change to `{{ s.command.command | e }}`.
12. **Hostinfo Error — title/name in `<h3>`** (line 243): same pattern as item 4. Change to `{{ s.command.title | e }}` / `{{ s.command.name | e }}`.
13. **Hostinfo Error — reason** (line 245): `{{ s.reason }}`. Change to `{{ s.reason | e }}`. (This is Requirement 13.)
14. **Hostinfo Error — command string** (line 247): `{{ s.command.command }}`. Change to `{{ s.command.command | e }}`.
15. **Command Results evidence section — stdout in `<pre>`** (line 281): `{{ s.stdout }}`. Change to `{{ s.stdout | e }}`.
16. **Command Results full output Success — stdout in `<pre>`** (line 300): `{{ s.stdout }}`. Change to `{{ s.stdout | e }}`.
17. **Command Results full output Failed — stdout in `<pre>`** (line 317): `{{ s.stdout }}`. Change to `{{ s.stdout | e }}`.

Expressions that are already escaped (do not change):
- `{{ s.command.name | e }}` in Coverage Gaps table (line 105) — already has `| e`.
- `{{ s.binary | e }}` (line 105) — already escaped.
- `{{ s.command.install_hint | e ... }}` (line 105) — already escaped.
- `{{ f.id | e }}` (line 123) — already escaped.
- `{{ f.summary | e }}` (line 125) — already escaped.
- All expressions in the Findings section with `| e` already applied.
- `{{ s.command.name | e }}` in evidence command result sections — already escaped.
- `{{ s.command.command | e }}` in evidence command result sections — already escaped.

Expressions that are safe without `| e` (numeric/controlled values — do not change):
- `{{ repetitions }}` — integer from config.
- `{{ max_parallel_commands }}` — integer from config.
- `{{ s.run_time_ms }}` — integer.
- `{{ s.value }}` in signals table — numeric signal value.
- `{{ s.stats.trend ... }}` — derived statistic.
- `{{ thr.value }}` — numeric threshold.
- `{{ loop.index }}` — loop counter.

**5b — `contrib/*.conf` discoverability**

In the `[defaults]` section of `contrib/linux.conf`, after existing uncommented keys, add:

```toml
# max_parallel_commands = 8
# repetitions = 1
# baseline_rolling_n = 10
```

Repeat identically in `contrib/osx.conf`.

**5c — README corrections**

Make the following targeted corrections in `README.md` (search by meaning, not line number):

1. Any phrase claiming `--exit-on crit` exits with code 1: change to exit code 2.
2. Any library API example showing `Analysis::new(config, rules)`: replace with a reference to `examples/json_report.rs` for the current constructor signature.
3. Any text claiming `install_hint` appears in `check` output: correct to `explain` output.
4. Any rolling baseline description implying it only appends when `--baseline` is passed: clarify it appends unconditionally each run.

**Phase complete when**: `cargo test --all-features` passes for all Phase 5 test files AND:
- `grep -c "exit.*1.*[Cc]rit\|[Cc]rit.*exit.*1" README.md` returns 0.
- Both `contrib/linux.conf` and `contrib/osx.conf` contain `# max_parallel_commands`.
- HTML rendering of a report with `reason = "<script>alert(1)</script>"` produces `&lt;script&gt;`.

### Test Scenarios

- GIVEN an `AnalysisReport` where a hostinfo error `reason` field is the string `<script>alert(1)</script>` WHEN the report is rendered to HTML using `html.j2` via `TemplateRenderer` THEN the output contains `&lt;script&gt;` AND does not contain a bare `<script>` tag. *(file: `tests/sdd_product_review_p5_c1_html_xss_fix.rs`)*
- GIVEN `contrib/osx.conf` is read WHEN the `[defaults]` section is inspected THEN it contains a commented-out line matching `# max_parallel_commands =`. *(verified by phase gate grep)*
- GIVEN `contrib/linux.conf` is read WHEN the `[defaults]` section is inspected THEN it contains a commented-out line matching `# max_parallel_commands =`. *(verified by phase gate grep)*
- GIVEN the README is read THEN no text asserts `--exit-on crit` exits with code 1 (grep for the pattern `exit.*1.*crit\|crit.*exit.*1` returns zero matches). *(verified by phase gate grep)*

---

## Decision Log

| Decision | Alternatives considered | Rationale |
|---|---|---|
| Add `PatternEngine::empty()` and `extend_from()` to `src/pattern/mod.rs` | Use `pub(crate)` on `patterns` field; add `merge` method | The `patterns` field is private. Two minimal methods give the fold pattern without exposing internals. `extend_from(&mut self, other)` avoids an allocation vs. `merge(self, other) -> Self`. |
| Embed pattern TOML via `include_str!` in `defaults::PATTERNS` | Runtime `load_from_dir` resolving installed path | Embedding is reproducible, requires no filesystem access, and mirrors how configs and templates are handled in the `defaults` module. |
| Use `PatternEngine::empty()` as fold accumulator with `extend_from()` in `Ok` branch | Collect `Ok` results and chain | Explicit fold with `empty()` accumulator is the direct expression of the resolution from sdd-validate. |
| `raw_excerpts` truncation as compile-time constant `MAX_EXCERPT_CHARS = 1_000` | Config key `[defaults] llm_excerpt_chars` | YAGNI — no current caller needs runtime control. |
| Add `Display` impls for `Aggregate` (in `src/cli/config.rs`) and `Unit` (in `src/signal.rs`) | Inline `match` arms at call site | Both enums are used in multiple contexts. A `Display` impl prevents future Debug-format recurrence. |
| Change `diff --output` field type to `OutputType` with clap `value_enum`; add `OutputType::Text` variant | Add runtime string validation | Compile-time validation is strictly safer; clap generates the "possible values" error message automatically. `Text` is excluded from root `--output` valid-values error to avoid confusion. |
| Handle `--output text` on root command with an explicit error arm in `generate_report` | Exclude `Text` from `value_variants()` for root `--output` | `value_enum` is applied to the root `--output` arg which uses the same `OutputType` enum. Keeping `Text` in `value_variants()` means clap accepts it; the `generate_report` match arm then returns a clear error. Excluding it via `#[value(skip)]` would require a different `OutputType` per context — more complexity for no benefit. |
| Leave `--exit-on crit → exit 2` as-is; fix only the README | Change code to exit 1 | An existing integration test asserts exit code 2. Changing the code would break it and existing scripts. |
| Scope `explain` signal ID lookup to config-derived signals | Full static catalogue of all collector signals | The config-derived list is always correct for the current install; a static catalogue can drift from collector reality. |
| Replace `InvalidConfig { reason: &'static str }` with three typed variants | Keep static reason string; add a `name: String` context field | Typed variants provide compile-time guarantees and carry runtime name context. |
| Move `--redact` warning and grammar fix into Phase 4 | Keep both in Phase 5 | Both are behaviour changes. Phase 5 is documentation and templates only; mixing behaviour changes violates single-reason-to-change. |
| Resolve `run_baseline` config loading as `Config::from_str(defaults::CONFIG).expect(...)` | Introduce `load_config_for_baseline()` helper | No new helper needed; the identical pattern already exists in `src/cli/mod.rs`. |
| Resolve `dirs` crate reference with explicit `std::env` code | Add `dirs` crate dependency | `dirs` is not a current dependency. Explicit `XDG_CONFIG_HOME` / `HOME` fallback achieves the same result with no new dependency. |
| `collect_signals_for_baseline` uses only `HostCollector` + `CpuCollector` | Use all eight collectors from `generate_report` | Baseline record is a lightweight capture. Using all eight collectors would pull in disk/network/etc. at record time without a corresponding reduction in the comparison set. The two collectors are guaranteed to return at least one signal on any host. |
| Use `CollectCtx { cgroup_path: None }` struct literal instead of `CollectCtx::default()` | Add `#[derive(Default)]` to `CollectCtx` | The struct literal is always correct regardless of whether `Default` is derived. Avoids an unverified assumption about `CollectCtx`'s trait impls. |
| Use `report.command_results().first().map(|v| v.as_slice()).unwrap_or(&[])` for raw_excerpts | Use `.iter().flatten()` to iterate all repetitions | `first()` matches the existing `run_diagnostics` behavior and processes only the first repetition. Using `.iter().flatten()` would include all repetitions, potentially producing duplicate excerpts. Consistency with existing code is more important than completeness here. |
| `known_topics_list` sorted alphabetically, newline-separated, uncapped | Grouped by type; truncated at N entries | Simple and consistent. No strong reason to group or truncate for a feature used interactively. |

---

## Open Decisions

None.

---

## Out of Scope

- Implementing `tcpretrans` / `execsnoop` signal data (`BpfCollector` returns `None` for these; fixing requires BCC integration work).
- `--workload` / `--profile` terminology unification (naming change that would break configs).
- `--show-profiles` promoted to a true subcommand (CLI shape change; deferred).
- Flamegraph reference in the Markdown template (template design decision, not a bug).
- `Severity` Display impl (tangential to this review).
- `swap_usage` USE dimension in `osx.conf` (separate macOS compatibility work).
- User-overridable pattern files on disk (embedded patterns are sufficient for this remediation; override support is a future enhancement).
