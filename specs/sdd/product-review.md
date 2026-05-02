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
- **Test strategy**: integration tests under `tests/` named `sdd_product_review_p<N>_c<N>_<desc>.rs`. Unit tests inline in source modules. Write failing tests before implementing fixes.
- **Integration test `<name>` token**: `product_review` — all test files for this SDD use this prefix.

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
src/cli/mod.rs            — Phase 1: pattern + rules wiring, baseline record, explain signal lookup, explain unknown-topic error path, redact warning, plural fix
src/cli/config.rs         — Phase 4: error variants updated to carry profile/command name context; Phase 3: Display impls for Aggregate
src/signal.rs             — Phase 3: Display impl for Unit
src/llm.rs                — Phase 2: raw_excerpts population; add redact parameter to from_report
src/diff.rs               — Phase 2 + Phase 4: output field type change; render_text takes filename labels
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
tests/sdd_product_review_p3_c1_signal_id_resolved.rs
tests/sdd_product_review_p3_c2_extract_display_human_readable.rs
tests/sdd_product_review_p4_c1_profile_name_in_error.rs
tests/sdd_product_review_p4_c2_diff_shows_filenames.rs
tests/sdd_product_review_p4_c3_baseline_list_empty_state.rs
tests/sdd_product_review_p4_c4_output_invalid_lists_values.rs
tests/sdd_product_review_p5_c1_html_xss_fix.rs
tests/sdd_product_review_p5_c2_redact_warning.rs
```

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
    /// `MAX_EXCERPT_CHARS` is a compile-time constant defined in `src/llm.rs`:
    pub const MAX_EXCERPT_CHARS: usize = 1_000;

    /// Signature change: add `redact: bool` parameter.
    pub fn from_report(report: &AnalysisReport, redact: bool) -> Self { ... }
}
```

### Config error variants (`src/cli/config.rs`)

The `InvalidConfig { reason: &'static str }` variant must be replaced by three specific variants to carry runtime context:

```rust
pub enum Error {
    // replaces InvalidConfig for the "no such profile" case:
    NoSuchProfile { name: String },
    // replaces InvalidConfig for the "profile command not found" case:
    ProfileCommandNotFound { profile: String, command: String },
    // replaces InvalidConfig for the "hostinfo command not found" case:
    HostinfoCommandNotFound { command: String },
    // keep existing non-context variants unchanged
    ParseConfigFailed { source: toml::de::Error },
    ReadConfigFileFailed { path: PathBuf, source: std::io::Error },
    InvalidExtractPattern { command: String, pattern: String, reason: String },
}
```

`InvalidConfig { reason: &'static str }` is removed entirely. All three call sites in `src/cli/config.rs` are updated.

### Display impls for Aggregate and Unit

Add `impl fmt::Display for Aggregate` in `src/cli/config.rs`:
- `Count` → `"count"`, `Last` → `"last"`, `Max` → `"max"`, `Min` → `"min"`, `Avg` → `"avg"`

Add `impl fmt::Display for Unit` in `src/signal.rs`:
- `Pct` → `"percent"`, `MillisPerOp` → `"ms"`, `BytesPerSec` → `"bytes/s"`, `Count` → `"count"`, `Iops` → `"iops"`, `Microseconds` → `"µs"`, `Hz` → `"hz"`, `Celsius` → `"celsius"`, `None` → `"none"`

### Pattern loading

`PatternEngine` does not have a `load_from_dir` method. Instead, use `PatternEngine::from_toml(text)` with embedded TOML strings via the existing `defaults` module pattern:

```rust
// In src/cli/mod.rs defaults module:
mod defaults {
    // existing ...
    pub(crate) static PATTERNS: &[&str] = &[
        include_str!("../../contrib/patterns/time_wait.toml"),
        include_str!("../../contrib/patterns/lock_contention.toml"),
        include_str!("../../contrib/patterns/nfs_stall.toml"),
        include_str!("../../contrib/patterns/slab_leak.toml"),
        include_str!("../../contrib/patterns/socket_leak.toml"),
        include_str!("../../contrib/patterns/thundering_herd.toml"),
    ];
}
```

In `generate_report`, build the engine from these slices. On parse error for any pattern file, log a warning and skip that pattern (do not hard-fail `generate_report`).

### diff subcommand output field

Change `Subcommand::Diff` in `src/cli/mod.rs`:
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

Callers pass `a_path.display().to_string()` and `b_path.display().to_string()`. The heading strings change from `"## Signals only in A"` to `"## Signals only in {label_a}"`.

---

## API Contracts

### `LlmOutput::from_report` (updated)

```
Input:  &AnalysisReport, redact: bool
Output: LlmOutput
Side effects: none
```

`raw_excerpts` is populated from `report.command_results().first()` (first repetition slice). For each `CommandResult::Success { command, stdout, .. }` entry, include an `LlmExcerpt { command: command.name().to_string(), output: stdout.chars().take(MAX_EXCERPT_CHARS).collect() }`. If `redact` is true, pass the assembled `LlmOutput` through `Redactor::from_env().redact_output(llm_out)` before returning.

All callers of `LlmOutput::from_report` must be updated to pass the `redact` flag:
- `run_convert` in `src/cli/mod.rs` (line ~698): `LlmOutput::from_report(&report, redact)`
- `generate_report` in `src/cli/mod.rs` (line ~1060): `LlmOutput::from_report(&report, opt.redact)`

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
| Malformed pattern TOML in `defaults::PATTERNS` | Embedded file is invalid TOML (compile-time authoring error) | `log::warn!` and skip that pattern; `generate_report` continues | Log line: `"pattern: failed to parse embedded pattern, skipping: {err}"` |
| `--output xyz` on root command | `FromStr` for `OutputType` returns `Err` | clap exits non-zero; error message appended with `"; valid values: template, html, json, markdown, llm"` | `"failed to parse xyz as output type; valid values: template, html, json, markdown, llm"` |
| `--output jsn` on `diff` subcommand | clap `value_enum` rejects unknown variant | clap exits non-zero with auto-generated error | `"error: invalid value 'jsn' for '--output <OUTPUT>'\n  [possible values: template, html, json, markdown, llm]"` |
| No such profile | `Config::profile()` called with unknown name | Return `Error::NoSuchProfile { name }` | `"no such profile 'nonexistent'"` |
| Profile command not found | `validate_profiles_commands` detects missing command | Return `Error::ProfileCommandNotFound { profile, command }` | `"profile 'p': command 'c' not found"` |
| Hostinfo command not found | `validate_host_info` detects missing command | Return `Error::HostinfoCommandNotFound { command }` | `"hostinfo: command 'c' not found"` |
| `explain` unknown topic | ID matches no command, rule, or signal | Return `Err(miette!(...))` with list of known topics | `"unknown topic 'x'; known topics: ..."` |
| `--redact` without `--output llm` | `opt.redact && opt.output != OutputType::Llm` | Print warning to stderr, continue | `"warning: --redact has no effect unless --output llm is also set"` |
| `baseline list` returns empty | `store.list()` returns `Ok(vec![])` | Print message, exit 0 | `"No baselines recorded."` |

---

## Implementation Phases

## Phase 1 — Wire pattern engine, user rules, and baseline record

Three fully-built subsystems are never called from the CLI. Fix all three in one phase because they all touch `generate_report` and `run_baseline` in `src/cli/mod.rs`. Each fix gets its own commit.

**Pattern engine wiring** (`generate_report` in `src/cli/mod.rs`):
1. Extend the `defaults` module with a `PATTERNS` static slice containing `include_str!` for each file in `contrib/patterns/` (list the six TOML files explicitly).
2. After workload rules are loaded and before `RuleEngine::new`, build the engine:
   ```rust
   let pattern_engine = defaults::PATTERNS.iter().fold(
       PatternEngine { patterns: vec![] },
       |mut acc, text| {
           match PatternEngine::from_toml(text) {
               Ok(pe) => { acc.patterns.extend(pe.patterns); acc }
               Err(e) => { log::warn!("pattern: failed to parse embedded pattern, skipping: {e}"); acc }
           }
       },
   );
   analysis = analysis.with_pattern_engine(pattern_engine);
   ```
   Note: `PatternEngine.patterns` is a private field; if direct construction is not possible, add a `PatternEngine::merge(self, other: PatternEngine) -> PatternEngine` method to `src/pattern/mod.rs`, or expose a `PatternEngine::empty() -> Self` constructor and a `extend` method. Prefer the minimal change.

**User rules wiring** (`generate_report` in `src/cli/mod.rs`):
- Replace `let mut all_rules = builtin_rules();` with:
  ```rust
  let user_rules_dir = dirs::config_dir()
      .map(|d| d.join("usereport").join("rules.d"));
  let mut loader = RulesLoader::new().with_builtins(builtin_rules());
  if let Some(dir) = user_rules_dir {
      loader = loader.with_user_dir(dir);
  }
  let rules_result = loader.load();
  // log any load errors but continue with successfully loaded rules
  let mut all_rules = rules_result.rules;
  ```
  The `dirs` crate is already a dependency (check `Cargo.toml`); if not, use `std::env::var("XDG_CONFIG_HOME")` fallback to `~/.config`.

**Baseline record signals** (`run_baseline` in `src/cli/mod.rs`):
- In `BaselineAction::Record { name }`, instead of calling `store.record(label, &[])`, run a minimal signal collection pass:
  ```rust
  let config = load_config_for_baseline()?; // reuse existing config-loading logic
  let signals = collect_signals_for_baseline(&config)?;
  store.record(label, &signals)?;
  ```
  Extract a private helper `collect_signals_for_baseline(config: &Config) -> miette::Result<Vec<Signal>>` that constructs a `HostCollector` + `CpuCollector` (the same set used in `generate_report`) and calls `.collect(ctx)` synchronously. Do not duplicate the full runner pipeline — only collector signals are needed for a baseline; command-extracted signals are optional. If collection fails, log a warning and record an empty slice rather than failing the command.

**Phase complete when**:
- A pattern TOML file in `contrib/patterns/` fires a finding on a synthetic signal set.
- A TOML rule file in `~/.config/usereport/rules.d/` fires findings.
- `usereport baseline record --name smoke` produces a baseline JSON with a non-empty `signals` map.

### Test Scenarios

- GIVEN a TOML pattern file in `contrib/patterns/` whose predicates match a provided synthetic signal set WHEN `Analysis` is constructed with that pattern engine and run THEN `report.findings()` contains a finding whose ID matches the pattern file entry.
- GIVEN a TOML rule file placed at `~/.config/usereport/rules.d/test.toml` with `id = "test.user_rule"` and an always-true predicate WHEN `generate_report` runs THEN the returned report contains a finding with `rule_id = "test.user_rule"`.
- GIVEN a config with at least one command that exits within 5 s WHEN `usereport baseline record --name smoke` runs THEN the baseline JSON on disk contains a `signals` object with at least one key whose value is a non-zero number.

---

## Phase 2 — LLM raw_excerpts and diff --output type safety

Two independent correctness gaps in separate files. Each sub-item is one commit.

**`src/llm.rs` — raw_excerpts**:
1. Change `raw_excerpts: Vec<String>` to `raw_excerpts: Vec<LlmExcerpt>`.
2. Add `pub struct LlmExcerpt { pub command: String, pub output: String }` (derive `Debug, Clone, Serialize, Deserialize`).
3. Change `from_report` signature to `pub fn from_report(report: &AnalysisReport, redact: bool) -> Self`.
4. Populate `raw_excerpts` from `report.command_results().first().unwrap_or(&[])`. For each `CommandResult::Success { command, stdout, .. }`, append `LlmExcerpt { command: command.name().to_string(), output: stdout.chars().take(Self::MAX_EXCERPT_CHARS).collect() }`.
5. If `redact` is true, apply `Redactor::from_env().redact_output(llm_out)` before returning. Move the redaction call from callers into `from_report`.
6. Update both call sites in `src/cli/mod.rs`: `LlmOutput::from_report(&report, opt.redact)` and `LlmOutput::from_report(&report, redact)`.
7. `convert` subcommand already accepts `--redact` (confirmed at `src/cli/mod.rs` line ~192); no new flag needed.

**`src/cli/mod.rs` + `src/diff.rs` — diff --output type safety**:
1. Change `Subcommand::Diff { output: String }` to `output: OutputType` with `#[arg(long, default_value = "text", value_enum)]`. Use `"text"` as the default (add `Text` variant to `OutputType` if not present, or use `"markdown"` if the existing text rendering maps to that; inspect `run_diff` to determine the correct default). Current `run_diff` uses `"json"` branch vs catch-all for text — map `"text"` to a new `OutputType::Text` variant or rename to align with existing enum variants. Concretely: if `OutputType` has no `Text` variant, add one; map the old catch-all text path to it.
2. Change `run_diff` signature to `fn run_diff(a_path: &PathBuf, b_path: &PathBuf, output: &OutputType) -> miette::Result<()>`.
3. Update the `FromStr` implementation's error message to append valid values: `Err(miette!("failed to parse {} as output type; valid values: template, html, json, markdown, llm", s))`.

**Phase complete when**:
- `usereport convert report.json --output llm` produces non-empty `raw_excerpts`.
- `usereport diff a.json b.json --output jsn` exits non-zero with clap printing "jsn" and "possible values".

### Test Scenarios

- GIVEN a JSON report where at least one command succeeded and emitted stdout WHEN `usereport convert report.json --output llm` runs THEN the output JSON `raw_excerpts` array is non-empty and each entry has a non-empty `output` field.
- GIVEN a JSON report where a command's stdout contains the literal string `myhostname` and redaction covers hostnames WHEN `usereport convert report.json --output llm --redact` runs THEN no `raw_excerpts[*].output` entry contains the literal string `myhostname`.
- GIVEN `usereport diff a.json b.json --output jsn` WHEN the command runs THEN exit code is non-zero and stderr contains both `jsn` and at least one valid format name such as `markdown`.

---

## Phase 3 — explain: signal ID support and Debug format fix

Two gaps in `run_explain` / `run_explain_command` in `src/cli/mod.rs`. Each sub-item is one commit.

**Signal ID lookup** (`run_explain` function):
After the existing command-name lookup and before the rule-ID lookup, add a third path: collect all `extract.signal_id` values from `config.commands` into a `HashSet<&str>`. If `id` matches any signal ID, find the command(s) that emit it and print a description:
```
Signal ID: mem.free_pct
Emitted by: mem-check (command)
Unit: percent
Aggregate: last
Pattern: (?P<val>\d+)
```
Use the `Display` impls added for `Aggregate` and `Unit` (Phase 3 prerequisite). A signal may be emitted by multiple commands; list all.

**Debug format fix** (`run_explain_command` function):
The line `"Extract: {} ({:?} {:?}) pattern={}"` at the `writeln!` in `run_explain_command` uses `{:?}` for `extract.aggregate` and `extract.unit`. Replace with `{}` (using the `Display` impls). Also add `Display` impls for `Aggregate` (`src/cli/config.rs`) and `Unit` (`src/signal.rs`) as specified in the Data Models section.

**Unknown topic error path** (`run_explain` function):
Replace the `eprintln!` + `process::exit(1)` block with:
```rust
return Err(miette!(
    "unknown topic '{}'\n\nKnown topics:\n{}",
    id,
    known_topics_list
));
```
where `known_topics_list` is built from commands, rules, and signal IDs. This makes `run_explain` consistent with all other error paths that return `miette::Result`.

**Phase complete when**:
- `usereport explain <signal-id>` exits 0 and prints a human-readable description.
- `usereport explain <extract-command>` prints `aggregate: last` (not `(Last`) when applicable.

### Test Scenarios

- GIVEN a running config that defines a signal with ID `mem.free_pct` WHEN `usereport explain mem.free_pct` runs THEN exit code is 0 and stdout contains the string `mem.free_pct`.
- GIVEN a config command with an `[[command.extract]]` block specifying `aggregate = "last"` and `unit = "percent"` WHEN `usereport explain <that-command-name>` runs THEN stdout contains the lowercase strings `last` and `percent` and does not contain the substring `(Last` or `Percent)`.

---

## Phase 4 — Error message quality

A cluster of missing-context error messages plus two behaviour improvements moved from Phase 5. Each sub-item is one commit.

**Config error context** (`src/cli/config.rs`):
Replace `InvalidConfig { reason: &'static str }` with three typed variants as specified in the Data Models section. Update:
- `Config::profile()`: use `Error::NoSuchProfile { name: profile_name.to_string() }`.
- `validate_host_info()`: use `Error::HostinfoCommandNotFound { command: c.clone() }`.
- `validate_profiles_commands()`: use `Error::ProfileCommandNotFound { profile: p.name.clone(), command: c.clone() }`.
- Update all existing unit tests that assert on these error strings.

**diff filenames** (`src/diff.rs`):
Change `render_text` signature to `pub fn render_text<W: std::io::Write>(d: &DiffReport, label_a: &str, label_b: &str, mut w: W) -> std::io::Result<()>`. Replace the four `"## ... in A"` / `"## ... in B"` heading literals with `format!("## ... in {label_a}")` / `format!("## ... in {label_b}")`. Update the call in `run_diff` to pass `a_path.display().to_string()` and `b_path.display().to_string()`.

**baseline list empty state** (`run_baseline` in `src/cli/mod.rs`):
In the `BaselineAction::List` arm, after collecting the list, add:
```rust
if names.is_empty() {
    println!("No baselines recorded.");
}
```

**`--output` invalid value with valid names** (`src/cli/mod.rs`):
The `FromStr for OutputType` already has an error path at `_ => Err(miette!("failed to parse {} as output type", s))`. Append `"; valid values: template, html, json, markdown, llm"` to that message. (This applies to the root command's `--output`; the `diff` subcommand's `--output` uses `value_enum` so clap generates its own message.)

**Grammar fix** (`src/cli/mod.rs`):
In `main()`, replace `eprintln!("{} binary/binaries not found", missing)` with:
```rust
eprintln!("{} {} not found", missing, if missing == 1 { "binary" } else { "binaries" });
```

**`--redact` warning** (`src/cli/mod.rs`):
In the `Opt::validate` method (or in `main()` before dispatch), add:
```rust
if opt.redact && opt.output != OutputType::Llm {
    eprintln!("warning: --redact has no effect unless --output llm is also set");
}
```
Check whether `redact` is also present on the `Convert` subcommand variant in the clap definition; if so, add the same check in `run_convert`. The two locations in the original SDD (lines 213–215 and 191–193) correspond to the root `Opt` and the `Convert` subcommand respectively — update both.

**Phase complete when**: Each of the five error paths above produces output matching the test scenarios below.

### Test Scenarios

- GIVEN a config that does not define a profile named `nonexistent` WHEN `usereport --profile nonexistent` runs THEN exit code is non-zero and stderr contains both the word `profile` and the string `nonexistent`.
- GIVEN two JSON files named `before.json` and `after.json` with at least one signal present in one but not the other WHEN `usereport diff before.json after.json` produces text output THEN the output contains the substrings `before.json` and `after.json` as section headings.
- GIVEN no baselines stored in the baseline directory WHEN `usereport baseline list` runs THEN exit code is 0 and stdout contains the words `No` and `baseline`.
- GIVEN `usereport --output xyz` with an unrecognised format value WHEN the command runs THEN stderr contains at least one valid format name such as `markdown`.

---

## Phase 5 — Documentation fixes and minor polish

No behaviour-changing code changes except the HTML template XSS fix. README and config file edits.

**HTML XSS** (`contrib/html.j2`):
Find the hostinfo error section (search for `s.reason`). The unescaped `{{ s.reason }}` must become `{{ s.reason | e }}`. Audit all other `{{ ... }}` expressions in `html.j2` that render user-supplied strings and add `| e` to any that lack it — this is a full template audit, not just the one occurrence.

**`contrib/*.conf` discoverability**:
In the `[defaults]` section of both `contrib/linux.conf` and `contrib/osx.conf`, add after existing uncommented keys:
```toml
# max_parallel_commands = 8
# repetitions = 1
# baseline_rolling_n = 10
```

**README corrections** (search for these current strings, not line numbers):
1. Any phrase claiming `--exit-on crit` exits with code 1 → change to exit code 2.
2. Library API example showing `Analysis::new(config, rules)` → replace with example matching actual `Analysis` constructor (point reader to `examples/json_report.rs`).
3. Any text claiming `install_hint` appears in `check` output → correct to `explain` output.
4. Rolling baseline description implying it only appends when `--baseline` is passed → clarify it appends unconditionally.
5. Baseline outlier finding demo output → update to match actual `"X standard deviations"` phrasing.

**Phase complete when**:
- HTML template renders `&lt;script&gt;` for a `<script>` payload in `s.reason`.
- Both default config files contain a commented `max_parallel_commands` line.
- `grep -n "exit.*1.*[Cc]rit\|[Cc]rit.*exit.*1" README.md` returns no matches.
- `usereport --output markdown --redact` prints a warning to stderr.

### Test Scenarios

- GIVEN an HTML report rendered from a command result whose `reason` field is `<script>alert(1)</script>` WHEN the rendered HTML is inspected THEN it contains `&lt;script&gt;` and does not contain a bare `<script>` tag.
- GIVEN `usereport --output markdown --redact` is invoked WHEN the command runs THEN stderr contains a warning referencing `--redact` and indicating it has no effect.
- GIVEN `contrib/osx.conf` is opened WHEN the `[defaults]` section is inspected THEN it contains a commented-out line for `max_parallel_commands`.

---

## Decision Log

| Decision | Alternatives considered | Rationale |
|---|---|---|
| Embed pattern TOML via `include_str!` in `defaults::PATTERNS` | Runtime `load_from_dir` resolving installed path | Embedding is reproducible and requires no filesystem access; it mirrors how configs and templates are already handled in the `defaults` module. User cannot currently override patterns, which is acceptable given Out of Scope constraints. |
| `raw_excerpts` truncation as compile-time constant `MAX_EXCERPT_CHARS = 1_000` in `src/llm.rs` | Config key `[defaults] llm_excerpt_chars` | YAGNI — no current caller needs runtime control. Constant is trivially adjustable in source and avoids adding a config key with no other callers. |
| Add `Display` impls for `Aggregate` (in `src/cli/config.rs`) and `Unit` (in `src/signal.rs`) | Inline `match` arms at the call site in `run_explain_command` | Both enums are used in multiple places (`extract.rs`, `cli/mod.rs`). A `Display` impl prevents future recurrence of Debug-format output. |
| Change `diff --output` field type to `OutputType` with clap `value_enum` | Add runtime string validation | Compile-time validation is strictly safer; clap generates the correct "possible values" error message automatically. |
| Leave `--exit-on crit → exit 2` as-is; fix only the README | Change code to exit 1 | `tests/sdd_version_2_phase1.rs:223` asserts exit code 2. Changing the code would break existing scripts. |
| Scope `explain` signal ID lookup to config-derived signals | Full static catalogue of all collector signals | The config-derived list is always correct for the current install; a static catalogue can drift from collector reality. |
| Replace `InvalidConfig { reason: &'static str }` with three typed variants | Keep static reason string; add a `name: String` context field | Typed variants provide compile-time guarantees and are more idiomatic in thiserror; the static reason string cannot carry runtime name context without `String` anyway. |
| Move grammar fix and `--redact` warning from Phase 5 into Phase 4 | Keep both in Phase 5 | Both are behaviour changes. Phase 5 is documentation and template only; mixing behaviour changes into it violates the single-reason-to-change principle and risks masking regressions. |
| Three CLI-wiring fixes in Phase 1 as separate commits | Single large commit | Co-location in `src/cli/mod.rs` justifies one phase, but each fix is logically independent. Separate commits make bisection possible. |

---

## Open Decisions

1. **`PatternEngine` patterns field visibility**: The `patterns` field of `PatternEngine` in `src/pattern/mod.rs` is private. The Phase 1 implementation needs to either expose a constructor (`PatternEngine::empty()` + `extend` method) or add a `merge` method. The minimal change must be chosen after inspecting the field visibility — this is an implementation detail, not a business decision. If `patterns` is `pub(crate)`, no change is needed; if private, add `pub fn empty() -> Self` and `pub fn extend_from(&mut self, other: PatternEngine)` to `src/pattern/mod.rs`.

---

## Out of Scope

- Implementing `tcpretrans` / `execsnoop` signal data (`BpfCollector` returns `None` for these; fixing requires BCC integration work).
- `--workload` / `--profile` terminology unification (naming change that would break configs).
- `--show-profiles` promoted to a true subcommand (CLI shape change; deferred).
- Flamegraph reference in the Markdown template (template design decision, not a bug).
- `Severity` Display impl (tangential to this review).
- `swap_usage` USE dimension in `osx.conf` (separate macOS compatibility work).
- User-overridable pattern files on disk (embedded patterns are sufficient for this remediation; override support is a future enhancement).
