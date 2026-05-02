# Changelog

## [0.2.0] - 2026-05-02

### Added
- macOS native signal collection via platform abstraction (cc89881)
- macOS: refine platform collectors and unify disk signal naming (445409f)
- Direct system collectors for CPU, memory, network, disk, interrupts, cgroups (5b2114f)
- Diagnostic rule engine with findings and severities (30e3ccf)
- Time-sampled collection with SampleStats (p50/p95/p99/min/max/trend) (727d0e2)
- Baselines and diff support with rolling JSONL auto-append (1ba59d2, 0a33c1d)
- Named workload rule packs and `--workload` flag (103f0ee)
- eBPF opt-in collectors: runqlat histogram, SampleStats p99 (25fca5a)
- dmesg miner and pattern catalog (3ccee4d)
- LLM-ready output format with redaction (36c1882)
- Coverage Gaps, Healthy section, threshold column in HTML reports (ff2e5fe)
- At-a-glance vital signs and USE coverage overview (e402207)
- Diagnostic thread linking findings to source commands (408b37b)
- Signal extraction from command stdout via regex (6dbe45c)
- `install_hint` and `what_to_look_for` fields on Command (0c27f3d)
- bpftrace fallback for `--profile-cpu` and `check` subcommand (7c54891)
- `net.estab_resets` signal (1230dac)
- Multi-progress spinners per command (dab0d20)
- Severity coloring, bold table headers, colored `--help` (57bfa79, e4f4944)
- Terminal Markdown rendering via termimad (3dd2112)
- USE method gap coverage in Linux and macOS profiles (52fe4cd)
- Rich error display at CLI boundary via miette (efb5825)
- Homebrew formula, Debian packages, cargo-dist for linux-musl + macOS arm (e15b7c5, 7e20ffb)

### Fixed
- macOS: locate iostat CPU columns from header instead of fixed indices (eced5e1)
- Drop analysis before handle.join() to prevent progress bar deadlock (7312a04)
- Gate bpf tools behind linux target; suppress them on macOS (dda1bc3)
- bpfcc tool names: `-bpfcc` suffix fallback for Ubuntu (bda8b41)
- html.j2 context.more iteration: use `| items` filter (27f945b)
- PatternEngine wired into Analysis pipeline (9b8bdfd)
- flamegraph_svg marked safe to prevent HTML escaping (254b708)
- cargo-dist tarball glob in deb package build step (ba79354)

### Changed
- Rule predicate parser rewritten with winnow 1.0 combinators (704e3ca)
- HTML template: host info and run config moved to top, findings below (00a0e14, ffa6f40)
- Markdown template reworked for terminal rendering (f48c169)
- Updated to Rust edition 2024, bumped all dependencies (86f2418)

## [0.1.4] - 2024

- Migrated from `structopt` to `clap v4`
- Migrated from `snafu` to `thiserror v2`
- Migrated from `uname` to `rustix` (makes `Context::new()` infallible)
- Migrated from `prettytable-rs` to `comfy-table v7`
- Migrated from `handlebars` to `minijinja v2`; templates use Jinja2 syntax (`.j2` extension)
- Migrated from `spectral` to `googletest v0.14` for tests
- Removed `failure`, `exitfailure`, `atty`, `shellwords`, `rustc-serialize` (CVE cleanup)
- Bumped `chrono` to ≥0.4.20, `tempfile` to ≥3.3 (clears RUSTSEC-2020-0159 and RUSTSEC-2023-0018)
- Fixed `Rc::get_mut` latent panics in `Command::exec`; merged Success/Failed arms with `drop(p)`
- Fixed `Command::terminate` to log warnings instead of panicking on kill/wait failure
- Fixed default timeout inconsistency: `Command::exec` now uses 5 s (matching `Defaults::default()`)
- Fixed `with_timeout` doc comment (was copy-pasted from `with_title`)
- Fixed `show_output_template` to return `anyhow::Result<()>` and propagate I/O errors
- Fixed `and_then(|p| Ok(...))` anti-pattern → `.map(...)`
- Deduplicated `defaults` module (single mod with per-OS `cfg` for `CONFIG`)
- Progress bar `JoinHandle` is now joined after analysis completes
- Debug output routed through `log::debug!` instead of raw `eprintln!`
- `RUST_LOG` value logged at startup when debug logging is active
- `CommandResult::Timeout` and `CommandResult::Error` logged at `warn!`
- HTML output now HTML-escapes user-derived values (`| e` filter)
- Added tests for `TemplateRenderer`, `JsonRenderer`, `create_command_filter`, `Opt::validate`, `ThreadRunner` result ordering, and `Analysis::run` integration

## [0.1.3] - 2024

- Bump minimum Rust version to 1.40.0
- Buffer command output into temporary files (avoids 64 KB pipe limit)

## [0.1.2] - 2024

- Initial public release
