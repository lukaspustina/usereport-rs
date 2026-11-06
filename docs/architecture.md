# usereport: architecture

Loaded on demand from `CLAUDE.md`.

## Layout

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

## Why there is no fast-compile recipe

`check` used to be `cargo check`, the contract resolver preferred that name, and so every
attestation proved that the tests compile rather than that they pass.
