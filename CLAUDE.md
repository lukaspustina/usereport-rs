# CLAUDE.md

## Build and test

Always pass `--all-features` — the binary, table rendering, and progress bar are all feature-gated under the `bin` feature:

```sh
cargo build --all-features
cargo test --all-features
cargo check --all-features
```

## Architecture

```
src/
  lib.rs          — public re-exports
  command.rs      — Command struct + CommandResult enum; exec() runs a subprocess via tempfile
  analysis.rs     — Analysis orchestrates runner; AnalysisReport + Context (hostname, uname, datetime)
  runner.rs       — Runner trait; ThreadRunner runs commands in parallel chunks, sorts by insertion order
  renderer.rs     — Renderer trait; TemplateRenderer (minijinja) + JsonRenderer
  cli/
    mod.rs        — clap v4 CLI; Opt, OutputType, generate_report
    config.rs     — TOML config: Config, Defaults, Profile, Hostinfo
  bin/
    usereport.rs  — entry point: calls usereport::cli::main()
contrib/
  osx.conf        — default macOS config (TOML)
  linux.conf      — default Linux config (TOML)
  html.j2         — default HTML report template (Jinja2)
  markdown.j2     — default Markdown report template (Jinja2)
```

## Key design points

- `Command::exec` uses an `Rc<tempfile::TempFile>` shared with subprocess for stdout capture; the `Rc` is dropped (`drop(p)`) before reading so `Rc::get_mut` is always safe.
- `Context::new()` is infallible (uses `rustix::system::uname()`).
- Templates use minijinja v2 with auto-escape disabled (HTML escaping done explicitly with `| e`).
- The custom `rfc2822` filter converts ISO-8601 timestamps to RFC 2822 for display.
