# SDD: Modernize UX/UI

Status: Implemented
Original: specs/sdd/modernize-ux-ui.md
Refined: 2026-05-02

## Overview

`usereport` produces rich, structured output but presents it with no visual hierarchy in the terminal. Raw markdown goes to stdout, severity levels are indistinguishable from regular text, and the progress bar is opaque about what is actually running. This SDD covers targeted improvements to terminal presentation without changing any output formats, data models, or library-crate dependencies.

## Context & Constraints

- **Stack:** Rust 2024 edition; `bin` feature gates all CLI deps (clap, indicatif, comfy-table, env_logger, human-panic, inferno).
- **Key file:** `src/cli/mod.rs` — orchestrates rendering and all human-facing output; most changes land here.
- **Key file:** `src/runner.rs` — `ThreadRunner` drives progress; currently sends bare `usize` counts over MPSC.
- **Key file:** `src/renderer.rs` — `TemplateRenderer` returns a `String` via a `Box<dyn Write>`; rendering stays unchanged, tty detection happens downstream.
- **Key file:** `src/bin/usereport.rs` — binary entry point; replace `anyhow::Result` with `miette::Result`.
- **Convention:** all new deps go under the `bin` feature in `Cargo.toml`; the library crate (`src/lib.rs` and all non-cli modules) must not gain new deps.
- **Convention:** tty detection at the output boundary using `std::io::IsTerminal` (stable since Rust 1.70, already imported at `src/cli/mod.rs` line 25 — do not add a duplicate import); piped output and `-O` file output must be byte-identical to the pre-change baseline.
- **anyhow removal scope:** `src/cli/mod.rs` currently imports `use anyhow::{Context as _, anyhow};` and every function returning `anyhow::Result`. `OutputType::from_str` returns `anyhow::Error`. All of these are migrated in Phase 4. `anyhow` is removed entirely from `Cargo.toml` (both `bin` feature and `[dependencies]`); it is not present in `[dev-dependencies]` today and tests do not use it directly.
- **`termimad::print_text` writes directly to stdout;** `writer` is intentionally unused on the tty path because `is_tty=true` implies `output_file.is_none()`. The tty render path is therefore integration/manual-only for test purposes.

## Architecture

```
src/cli/mod.rs
  generate_report()
    └─ renderer.render() → String (captured via BufWriter to Vec<u8>)
    └─ render_for_output(rendered: &str, writer: &mut dyn Write, is_tty: bool, format: &OutputType) -> miette::Result<()>
        — calls termimad::print_text(rendered) when is_tty=true && format==Markdown  (writes to stdout directly)
        — writes raw string to writer otherwise

  run_explain()   ← owo-colors severity coloring, gated on stdout.is_terminal()
  run_check()     ← comfy_table::Color status coloring, gated on stdout.is_terminal()
  show_profiles() ← comfy_table bold header cells
  show_commands() ← comfy_table bold header cells

src/runner.rs
  ProgressEvent { seq: usize, name: String }  (pub(crate))
  ThreadRunner::with_progress(self, progress_tx: impl Into<Option<Sender<ProgressEvent>>>) -> Self
  create_child: sends ProgressEvent { seq, name, kind: Started } before exec(), ProgressEvent { seq, name, kind: Finished } after
  MultiProgress + spinners: owned by the progress thread spawned in cli/mod.rs

src/bin/usereport.rs
  main() -> miette::Result<()>
  miette::set_hook(...)  ← fancy renderer
  setup_panic!()         ← human-panic stays, orthogonal to miette
```

No changes to `src/renderer.rs`, `src/analysis.rs`, `src/command.rs`, or any Jinja2 template.

## Requirements

1. The system shall render Markdown output with color and visual hierarchy when stdout is a terminal (`stdout.is_terminal() == true`) and no `-O` file is given; raw Markdown in all other cases (pipe, `-O` file, non-markdown formats).
2. The `explain` command shall print severity labels in color when stdout is a terminal: Crit → red, Warn → yellow, Info → blue, Ok → green; no ANSI sequences when stdout is not a terminal.
3. The `check` subcommand shall color-code the Status column when stdout is a terminal: `ok` → green, any non-ok value → red; no ANSI sequences when stdout is not a terminal.
4. The `--show-profiles` and `--show-commands` tables shall use bold header row cells via `comfy_table::Attribute::Bold`.
5. The progress display shall show the name of each currently-running command alongside the overall count bar; spinners are added when a command starts and cleared when it finishes.
6. The system shall replace `anyhow` at the CLI boundary with `miette` for source-highlighted, human-friendly error messages; the library crate is unaffected.
7. All tty detection shall use `std::io::IsTerminal`; no additional crate is needed for that detection.

## File & Module Structure

| File | Change | Version / notes |
|---|---|---|
| `Cargo.toml` | Add `termimad`, `owo-colors`, `miette` under `bin` feature; remove `anyhow` from `bin` feature and from `[dependencies]` entirely | `termimad = { version = "0.34", optional = true }`, `owo-colors = { version = "4", optional = true }`, `miette = { version = "7", features = ["fancy"], optional = true }` |
| `src/cli/mod.rs` | tty detection + `render_for_output`; owo-colors in `run_explain`; comfy_table Color in `run_check`; bold headers in `show_profiles`/`show_commands`; MultiProgress wiring in `create_progress_bar`; migrate all `anyhow::Result` to `miette::Result` | `use std::io::IsTerminal` is already present at line 25 — do not add a duplicate |
| `src/runner.rs` | Add `ProgressEvent` struct and `EventKind` enum; change `with_progress` and `create_child` to use `Sender<ProgressEvent>`; send Started before exec, Finished after | `pub(crate)` visibility; no new deps |
| `src/bin/usereport.rs` | `fn main() -> miette::Result<()>`; call `miette::set_hook`; keep `setup_panic!()` | `use miette::IntoDiagnostic;` as needed |

## Data Models

```rust
// src/runner.rs — new types, pub(crate)

#[derive(Debug, Clone)]
pub(crate) enum EventKind {
    Started,
    Finished,
}

#[derive(Debug, Clone)]
pub(crate) struct ProgressEvent {
    pub seq: usize,
    pub name: String,
    pub kind: EventKind,
}
```

Updated `ThreadRunner` field:

```rust
// src/runner.rs — field type change
pub struct ThreadRunner {
    progress_tx: Option<Sender<ProgressEvent>>,
}

// with_progress new signature
pub fn with_progress<T: Into<Option<Sender<ProgressEvent>>>>(self, progress_tx: T) -> Self { ... }
```

`create_child` sends two events:

```rust
// Inside spawn closure, before command execution:
if let Some(ref tx) = progress_tx {
    tx.send(ProgressEvent { seq, name: name.clone(), kind: EventKind::Started })
      .expect("Thread failed to send progress via channel");
}
let result = command.exec();
// After command execution:
if let Some(ref tx) = progress_tx {
    tx.send(ProgressEvent { seq, name: name.clone(), kind: EventKind::Finished })
      .expect("Thread failed to send progress via channel");
}
```

`create_progress_bar` new return type:

```rust
// src/cli/mod.rs
fn create_progress_bar(expected: usize) -> (Sender<ProgressEvent>, JoinHandle<()>)
```

`render_for_output` signature:

```rust
// src/cli/mod.rs
fn render_for_output(
    rendered: &str,
    writer: &mut dyn Write,
    is_tty: bool,
    format: &OutputType,
) -> miette::Result<()>
```

## Implementation Phases

## Phase 1 — Terminal Markdown Rendering

Add to `Cargo.toml` under the `bin` feature:

```toml
termimad = { version = "0.34", optional = true }
```

In `generate_report()`, capture the renderer output into a `String` (render to a `Vec<u8>` then convert), then call `render_for_output` with:
- `is_tty = opt.output_file.is_none() && std::io::stdout().is_terminal()`
- `format = &opt.output`
- the existing `writer` from `output_writer()`

`render_for_output` implementation:

```rust
fn render_for_output(
    rendered: &str,
    writer: &mut dyn Write,
    is_tty: bool,
    format: &OutputType,
) -> miette::Result<()> {
    if is_tty && *format == OutputType::Markdown {
        // termimad::print_text writes directly to stdout;
        // writer is intentionally unused here because is_tty=true implies output_file.is_none()
        termimad::print_text(rendered);
    } else {
        write!(writer, "{}", rendered).into_diagnostic()?;
    }
    Ok(())
}
```

`use std::io::IsTerminal` is already present at `src/cli/mod.rs` line 25 — do not add a duplicate.

No changes to `src/renderer.rs`. The termimad default skin is used (no `MadSkin` customization).

Phase complete when: `usereport` in a terminal shows colored headings, bold text, and styled code fences; `usereport | cat` produces byte-identical raw Markdown; `usereport -O /tmp/out.md` produces raw Markdown in the file; `usereport --output html` on a tty produces raw HTML.

### Test Scenarios

- GIVEN `is_tty=false` and `format=Markdown` WHEN `render_for_output` is called with a captured `Vec<u8>` writer THEN the bytes written equal the raw input string with no bytes matching `\x1b[`.
- GIVEN `is_tty=true` and `format=Html` WHEN `render_for_output` is called with a captured writer THEN the bytes written equal the raw renderer string with no bytes matching `\x1b[`.
- GIVEN `is_tty=true` and `format=Markdown` WHEN `render_for_output` is called THEN the function returns `Ok(())` with no error. Note: the tty render path bypasses the writer (termimad writes to stdout directly); this path is integration/manual-only — verify by running the binary in a terminal and confirming colored output appears.
- GIVEN `output_file=Some(path)` and `format=Markdown` (forcing `is_tty=false` because `output_file.is_some()`) WHEN the binary writes its report THEN the file bytes contain no `\x1b[` and match the raw `TemplateRenderer` output.

## Phase 2 — Colored Diagnostics in Plain-text Commands

Add to `Cargo.toml` under the `bin` feature:

```toml
owo-colors = { version = "4", optional = true }
```

No `features` change is needed for `comfy-table`; `Cell::fg` and `Cell::add_attribute` are available in the default feature set (`tty` is on by default).

**`run_explain`:** Apply color only when `std::io::stdout().is_terminal()`. Extract the severity label first, then apply `owo-colors`:

```rust
let label = format!("{:?}", rule.severity);
if is_tty {
    use owo_colors::OwoColorize as _;
    let colored = match rule.severity {
        Severity::Crit => label.red().to_string(),
        Severity::Warn => label.yellow().to_string(),
        Severity::Info => label.blue().to_string(),
        Severity::Ok   => label.green().to_string(),
    };
    println!("Severity: {colored}");
} else {
    println!("Severity: {label}");
}
```

`owo-colors` is used only for plain-text line output in `run_explain`; it is not used for table cells.

**`run_check`:** Apply `comfy_table::Cell::fg(comfy_table::Color::Green)` / `Cell::fg(comfy_table::Color::Red)` for the Status column only when `std::io::stdout().is_terminal()`. `comfy_table::Color` is a standalone enum; `owo-colors` types are not used here.

**`show_profiles` / `show_commands`:** Set header cells bold:

```rust
use comfy_table::Attribute;
// for each header cell:
Cell::new("Column Name").add_attribute(Attribute::Bold)
```

`Attribute` is re-exported from `comfy-table` (backed by `crossterm`).

Phase complete when: `usereport explain mem.pressure` on a tty prints `Warn` in yellow; `usereport check` on a tty prints `ok` in green and non-ok in red; table headers are bold; piped output contains no ANSI sequences.

### Test Scenarios

- GIVEN a rule with `severity=Warn` and `is_tty=true` injected as a parameter WHEN `run_explain` renders the severity label THEN the captured output contains `\x1b[` surrounding the token "Warn".
- GIVEN a rule with `severity=Ok` and `is_tty=true` WHEN `run_explain` renders the severity label THEN the captured output contains `\x1b[` surrounding the token "Ok".
- GIVEN `is_tty=false` WHEN `run_explain` renders any severity label THEN the captured output contains no `\x1b[`.
- GIVEN at least one binary is present (`status="ok"`) and `is_tty=true` WHEN `run_check` renders the table THEN the "ok" cell value is preceded by `\x1b[32m` in the captured output.
- GIVEN at least one binary is absent and `is_tty=true` WHEN `run_check` renders the table THEN the non-ok cell value is preceded by `\x1b[31m` in the captured output.
- GIVEN any config and `is_tty=true` WHEN `show_profiles` renders the table THEN the header row bytes contain `\x1b[1m` (Bold attribute).

## Phase 3 — Rich Progress with Running Command Names

**`src/runner.rs` changes:**

1. Add `EventKind` enum and `ProgressEvent` struct (see Data Models).
2. Change `ThreadRunner.progress_tx` field type from `Option<Sender<usize>>` to `Option<Sender<ProgressEvent>>`.
3. Update `with_progress` signature accordingly.
4. In `create_child`, send `EventKind::Started` before `command.exec()` and `EventKind::Finished` after it returns (see Data Models for exact send pattern).
5. Spinner map key: use `event.seq` (`usize`) from `ProgressEvent` to avoid collision when two commands share the same name.

**`src/cli/mod.rs` — `create_progress_bar` rewrite:**

```rust
fn create_progress_bar(expected: usize) -> (Sender<ProgressEvent>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<ProgressEvent>();
    let handle = thread::spawn(move || {
        let mp = indicatif::MultiProgress::new();
        let count_bar = mp.add(indicatif::ProgressBar::new(expected as u64));
        count_bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{bar:40} {pos}/{len} commands")
                .unwrap(),
        );
        // spinner map: seq → ProgressBar (active spinners only)
        let mut spinners: std::collections::HashMap<usize, indicatif::ProgressBar> = Default::default();

        for event in rx {
            match event.kind {
                EventKind::Started => {
                    let s = mp.add(indicatif::ProgressBar::new_spinner());
                    s.set_message(event.name.clone());
                    s.enable_steady_tick(std::time::Duration::from_millis(80));
                    spinners.insert(event.seq, s);
                }
                EventKind::Finished => {
                    if let Some(s) = spinners.remove(&event.seq) {
                        s.finish_and_clear();
                    }
                    count_bar.inc(1);
                }
            }
        }

        count_bar.finish_and_clear();
    });
    (tx, handle)
}
```

**`MultiProgress` ownership:** `MultiProgress` is created inside the progress thread. It does not live in `cli/mod.rs` at the call site. This keeps presentation concerns inside `create_progress_bar`.

Phase complete when: running `usereport --progress --profile net` shows individual command spinners (e.g. `⣷ nettop_snapshot`) that appear while the command runs and disappear on completion, alongside the overall count bar; `--no-progress` produces no progress output on stderr.

### Test Scenarios

- GIVEN a `ProgressEvent { seq: 0, name: "cmd_a".into(), kind: EventKind::Started }` sent over an MPSC channel WHEN the progress consumer processes it THEN `spinners` contains an entry for `seq=0`.
- GIVEN a subsequent `ProgressEvent { seq: 0, name: "cmd_a".into(), kind: EventKind::Finished }` WHEN the consumer processes it THEN `spinners.get(&0)` returns `None` and `count_bar` position equals 1.
- GIVEN two Started/Finished pairs for `seq=0` and `seq=1` sent in order WHEN the consumer drains the channel THEN `spinners` map is empty and `count_bar` position equals 2.
- GIVEN `--no-progress` WHEN `create_runner` is called THEN `create_progress_bar` is not invoked and the returned `JoinHandle` is `None`.
- GIVEN all senders are dropped (channel closed) WHEN the consumer loop exits THEN `count_bar.finish_and_clear()` is called exactly once.
- GIVEN a command is dispatched WHEN its spinner is active THEN the spinner shows the command name alongside the count bar (integration/manual: indicatif renders to stderr; verify by running with `--progress`).

## Phase 4 — Miette Error Reporting

**Prerequisite — demote `output_writer` visibility:**

Before migrating return types, change `pub fn output_writer` at `src/cli/mod.rs` line ~749 to `pub(crate) fn output_writer`. Update both unit tests that call it (`test_output_writer_none_returns_writer`, `test_output_writer_some_creates_parent_dirs`) to accept the new `miette::Result` return type after the migration below.

**`Cargo.toml` changes:**

- Remove `anyhow = { version = "1", optional = true }` from `[dependencies]` entirely.
- Add `miette = { version = "7", features = ["fancy"], optional = true }` to `[dependencies]`.
- Update the `bin` feature array: replace `"dep:anyhow"` (or `"anyhow"`) with `"dep:miette"` (or `"miette"`).
- `anyhow` is not in `[dev-dependencies]`; no change needed there.

**Import replacement in `src/cli/mod.rs`:**

```rust
// Remove:
use anyhow::{Context as _, anyhow};
// Add:
use miette::{miette, bail, IntoDiagnostic, Context as _};
```

**Complete migration checklist — all functions in `src/cli/mod.rs` returning `anyhow::Result`:**

| Function | File | Notes |
|---|---|---|
| `pub fn main()` | `src/cli/mod.rs` | Change to `pub fn main() -> miette::Result<()>` |
| `Opt::validate` | `src/cli/mod.rs` | Change to `miette::Result<()>` |
| `show_output_template` | `src/cli/mod.rs` | Change to `miette::Result<()>` |
| `run_subcommand` | `src/cli/mod.rs` | Change to `miette::Result<()>` |
| `run_baseline` | `src/cli/mod.rs` | Change to `miette::Result<()>`; also apply `.into_diagnostic()` at `serde_json::to_string_pretty(&record)?` (approx line 461) |
| `run_diff` | `src/cli/mod.rs` | Change to `miette::Result<()>`; apply `.into_diagnostic()` at `serde_json::from_slice` (approx line 476) and `serde_json::to_writer_pretty` (approx line 483) |
| `run_explain` | `src/cli/mod.rs` | Change to `miette::Result<()>` |
| `generate_flamegraph` | `src/cli/mod.rs` | Change to `miette::Result<()>` |
| `generate_svg_from_perf_script` | `src/cli/mod.rs` | Change to `miette::Result<()>`; apply `.into_diagnostic()` at `String::from_utf8(svg)?` (approx line 593) |
| `generate_report` | `src/cli/mod.rs` | Change to `miette::Result<()>` |
| `parse_duration` | `src/cli/mod.rs` | Change to `miette::Result<Duration>` |
| `create_commands` | `src/cli/mod.rs` | Change to `miette::Result<Vec<Command>>` |
| `create_renderer` | `src/cli/mod.rs` | Change to `miette::Result<Box<dyn Renderer>>` |
| `output_writer` | `src/cli/mod.rs` | Change to `pub(crate) fn output_writer(...) -> miette::Result<...>` |

**Substitution rules:**

- `anyhow!("msg")` → `miette!("msg")`
- `anyhow::bail!("msg")` → `bail!("msg")`
- `.context("msg")` (from `anyhow::Context`) → `.into_diagnostic().context("msg")`
- `OutputType::from_str` returns `anyhow::Error` today; change return type to `miette::Error` using `miette!`
- Any `SomeError` that does not implement `miette::Diagnostic` (e.g. `serde_json::Error`, `std::string::FromUtf8Error`, `std::io::Error`) requires `.into_diagnostic()` before `?`

**`src/bin/usereport.rs`:**

```rust
fn main() -> miette::Result<()> {
    setup_panic!();  // human-panic: handles panics, orthogonal to miette error returns
    miette::set_hook(Box::new(|_| Box::new(miette::MietteHandlerOpts::new().build()))).ok();
    usereport::cli::main()
}
```

`human-panic` and `miette` are orthogonal: `human-panic` intercepts `panic!` via a hook; `miette::Result` propagates `Err` returns through `main`. Both remain active.

Phase complete when: `usereport --config /nonexistent/path.toml` prints a miette-formatted error (fancy box-drawing on a tty); a valid run is byte-identical to the pre-miette baseline; `grep -r 'anyhow' src/cli/mod.rs src/bin/usereport.rs` produces no output.

### Test Scenarios

- GIVEN `--config /nonexistent/path.toml` WHEN the binary exits THEN exit code is non-zero and stderr contains the error message text (not a bare "Error:" prefix); verify with a process-level integration test.
- GIVEN stderr is a tty and `--config /nonexistent/path.toml` WHEN the binary exits THEN stderr contains miette fancy box-drawing characters (`╭` or `×`); this test requires a pty or is documented as a manual verification step.
- GIVEN a valid config and valid profile WHEN the binary runs to completion THEN exit code is 0 and stdout bytes are identical to the pre-miette baseline captured under identical inputs.
- GIVEN any migrated `miette!("msg")` call site WHEN the same invalid input is provided THEN the error message string in stderr is identical to the previous `anyhow` output.
- GIVEN Phase 4 is complete WHEN `grep -r 'anyhow' src/cli/mod.rs src/bin/usereport.rs` is run THEN the command produces no output.

## Decision Log

| Decision | Alternatives considered | Reason rejected |
|---|---|---|
| `termimad = "0.34"` for terminal markdown | `bat` as subprocess; custom ANSI rendering; `syntect` | `bat` requires external binary; custom rendering is too much code; `syntect` is heavy and overkill for prose markdown |
| Default `MadSkin` only (no customization) | Custom skin with tuned heading color / code-block background | YAGNI — default skin is adequate; skin tuning can be done post-implementation without SDD change |
| `owo-colors` for `run_explain` plain text | `nu-ansi-term`, `colored`, `termcolor` | `owo-colors` is zero-allocation, actively maintained, clean `.red()` API; `termcolor` is primarily for cross-platform file handles |
| `comfy_table::Color` for `run_check` table cells | Use `owo-colors` for table cells | `Cell::fg` takes `comfy_table::Color`, not an `owo-colors` type; mixing types would require string wrapping |
| `miette` replaces `anyhow` at CLI boundary only | Replace throughout; keep `anyhow` everywhere | Library crate should not depend on `miette`; `thiserror` types already exist and work well internally |
| tty detection via `std::io::IsTerminal` | `atty` crate; `termion` | Stable stdlib since Rust 1.70; zero additional deps |
| `MultiProgress` owned by progress thread in `cli/mod.rs` | Move to `src/runner.rs` | Progress rendering is a presentation concern; the thread already lives in `cli/mod.rs` via `create_progress_bar`; keeping it there avoids pushing a presentation dep into `runner.rs` |
| Spinner map keyed by `event.seq: usize` from `ProgressEvent` | Key by command name string | Two commands can share the same name in a chunk; name-keyed map would overwrite entries |
| `Started`/`Finished` `EventKind` variants | Single event on completion only | A single completion event means the spinner is shown and immediately cleared in the same tick — never visible while the command runs; the `EventKind` enum fixes the lifecycle so spinners appear before exec and disappear after |
| `finish_and_clear()` on completed spinners | `finish_with_message("done")` | Cleaner output: completed commands disappear rather than accumulating as "done" lines |
| Remove `ProgressEvent::done` and `ProgressEvent::total` fields | Keep them as metadata | Both were populated but never read by the consumer; YAGNI — removed in favor of `EventKind` which carries all needed state |
| `tracing` migration deferred | Replace `env_logger` now | Mostly developer-facing; no end-user UX impact; non-trivial migration |
| Interactive prompts out of scope | Add guided interactive mode | Contradicts the tool's design: fast, zero-interaction, CI-friendly |

## Open Decisions

None.

## Out of Scope

- TUI / ratatui full-screen interface
- Interactive mode or guided prompts (`inquire`, `dialoguer`)
- `tracing` migration (keep `log` + `env_logger`)
- Changing any output format (html, json, llm, template) — those remain unchanged
- Color in piped or `-O` file output
- `MadSkin` customization for termimad
- Changes to `src/renderer.rs` or any Jinja2 template
