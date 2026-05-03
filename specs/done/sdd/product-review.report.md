# SDD Implementation Report: product-review.md

**Date**: 2026-05-02
**Phases run**: 1, 2, 3, 4, 5
**Overall status**: all-shipped

| Phase | Title | Status | Commit |
|-------|-------|--------|--------|
| 1 | Wire pattern engine, user rules, and baseline record | shipped | 0558b29 |
| 2 | LLM raw_excerpts and diff --output type safety | shipped | 8916dd7 |
| 3 | explain signal ID support and Debug format fix | shipped | b02fb51 |
| 4 | Error message quality | shipped | c5ea399 |
| 5 | Documentation fixes and minor polish | shipped | fc7bca9 |

---

## Phase 1: Wire pattern engine, user rules, and baseline record

**Status**: shipped  
**Commit**: 0558b29

### Acceptance Criteria

| # | Criterion | Tests | Status |
|---|-----------|-------|--------|
| 1 | PatternEngine::empty() and extend_from() exist | sdd_product_review_p1_c1_pattern_engine_empty.rs | passing |
| 2 | Pattern engine wiring in generate_report | sdd_product_review_p1_c2_pattern_engine_wired.rs | passing |
| 3 | User rules wired from XDG_CONFIG_HOME | sdd_product_review_p1_c3_user_rules_wired.rs | passing |
| 4 | Baseline record uses real signals | (inline) | passing |

### Key changes
- Added `PatternEngine::empty()` and `extend_from()` to `src/pattern/mod.rs`
- Added `defaults::PATTERNS` static with all 6 embedded pattern TOML files
- Wired pattern engine into `generate_report` via `analysis.with_pattern_engine(...)`
- Replaced `let mut all_rules = builtin_rules()` with `RulesLoader` + XDG user dir
- Fixed `resolve_path` bug: `host.*` signals other than `host.cpu_count` were returning `None` early; fixed by narrowing the guard
- `run_baseline` now calls `collect_signals_for_baseline()` instead of `store.record(label, &[])`

---

## Phase 2: LLM raw_excerpts and diff --output type safety

**Status**: shipped  
**Commit**: 8916dd7

### Acceptance Criteria

| # | Criterion | Tests | Status |
|---|-----------|-------|--------|
| 1 | LlmExcerpt struct and raw_excerpts populated | sdd_product_review_p2_c1_llm_excerpts_populated.rs | passing |
| 2 | raw_excerpts truncated at MAX_EXCERPT_CHARS | sdd_product_review_p2_c2_llm_excerpt_truncation.rs | passing |
| 3 | Redaction applies to excerpt output field | sdd_product_review_p2_c3_llm_excerpt_redaction.rs | passing |
| 4 | OutputType::Text accepted for diff --output | sdd_product_review_p2_c4_diff_output_type.rs | passing |

### Key changes
- Added `LlmExcerpt { command: String, output: String }` to `src/llm.rs`
- Changed `raw_excerpts: Vec<String>` to `raw_excerpts: Vec<LlmExcerpt>`
- `from_report` gains `redact: bool` parameter; populates excerpts from command results
- Added `OutputType::Text` variant; excluded from root `--output` ValueEnum variants
- `diff --output` uses `value_parser` closure for FromStr-based parsing
- `diff::render_text` signature changed to accept `label_a`/`label_b` for headings

---

## Phase 3: explain signal ID support and Debug format fix

**Status**: shipped  
**Commit**: b02fb51

### Acceptance Criteria

| # | Criterion | Tests | Status |
|---|-----------|-------|--------|
| 1 | explain looks up signal IDs | sdd_product_review_p3_c1_explain_signal_id.rs | passing |
| 2 | explain uses Display for Aggregate/Unit | sdd_product_review_p3_c2_explain_display_format.rs | passing |
| 3 | explain respects --config flag | sdd_product_review_p3_c3_explain_config_flag.rs | passing |

### Key changes
- `run_explain` now searches signal IDs extracted from config commands via `cmd.extract()`
- `explain` subcommand dispatch moved before `run_subcommand` to access `opt.config`
- Added `impl fmt::Display for Aggregate` in `src/cli/config.rs`
- Added `impl fmt::Display for Unit` in `src/signal.rs`
- `run_explain` returns `Err(miette!(...))` instead of `eprintln!+process::exit(1)`

---

## Phase 4: Error message quality

**Status**: shipped  
**Commit**: c5ea399

### Acceptance Criteria

| # | Criterion | Tests | Status |
|---|-----------|-------|--------|
| 1 | "no such profile" includes profile name | sdd_product_review_p4_c1_error_no_such_profile.rs | passing |
| 2 | "profile command not found" includes both names | sdd_product_review_p4_c2_error_profile_cmd_not_found.rs | passing |
| 3 | "hostinfo command not found" includes command name | sdd_product_review_p4_c3_error_hostinfo_cmd.rs | passing |
| 4 | Binary plural fix: "1 binary" not "1 binaries" | sdd_product_review_p4_c4_binary_plural.rs | passing |
| 5 | baseline list prints "No baselines recorded." when empty | sdd_product_review_p4_c5_baseline_list_empty.rs | passing |
| 6 | --redact without --output llm prints warning | sdd_product_review_p4_c6_redact_warning.rs | passing |

### Key changes
- Added typed error variants `NoSuchProfile`, `ProfileCommandNotFound`, `HostinfoCommandNotFound` to `src/cli/config.rs`
- Fixed `{n} binary/binaries not found` singular/plural
- `baseline list` empty state now prints "No baselines recorded."
- `run_convert` and `Opt::validate` emit warning when `--redact` given without `--output llm`

---

## Phase 5: Documentation fixes and minor polish

**Status**: shipped  
**Commit**: fc7bca9

### Acceptance Criteria

| # | Criterion | Tests | Status |
|---|-----------|-------|--------|
| 1 | html.j2 XSS: s.reason escaped | sdd_product_review_p5_c1_html_xss_fix.rs | passing |

### Phase gates
- `grep -c "exit.*1.*[Cc]rit\|[Cc]rit.*exit.*1" README.md` → 0 ✓
- `contrib/linux.conf` contains `# max_parallel_commands` ✓
- `contrib/osx.conf` contains `# max_parallel_commands` ✓
- HTML render of `reason = "<script>alert(1)</script>"` produces `&lt;script&gt;` ✓

### Key changes
- Added `| e` to all user-supplied strings in `contrib/html.j2`: hostname, uname, stdout, reason, command strings, link urls/names, context.more keys/values
- Added commented `# repetitions`, `# max_parallel_commands`, `# baseline_rolling_n` to both conf files
- README: `--exit-on crit` exit code corrected 1→2, `install_hint` location corrected to `explain`, baseline description clarified, library API example replaced with reference to `examples/json_report.rs`

---

## Manual Test Plan

1. `cargo build --all-features` — expected: builds cleanly with no warnings
2. `cargo test --all-features` — expected: all suites green, 0 failures
3. `./target/debug/usereport --output html -O /tmp/test.html` then `grep '&lt;' /tmp/test.html` — expected: at least one escaped entity present
4. `./target/debug/usereport explain cpu.iowait_pct` — expected: shows command and signal details using human-readable "pct" and "avg/last/max/min/count" units
5. `./target/debug/usereport diff a.json b.json` — expected: section headings say "Signals only in a.json" not "Signals only in A"
6. `./target/debug/usereport baseline list` (no baselines stored) — expected: "No baselines recorded."
7. `./target/debug/usereport --output llm --redact` — expected: no warning on stderr (correct combination)
8. `./target/debug/usereport --output json --redact` — expected: warning on stderr about `--redact` without `--output llm`
