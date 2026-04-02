# Changelog

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
