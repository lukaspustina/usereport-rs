# CLAUDE.md

## Build and test

- Always through `just` (`just --list`), never `cargo` directly: the recipes pass `--all-features`
  (binary, table rendering and progress bar sit behind the `bin` feature).
- `just adlc-verify` is the gate (fmt-check, clippy, whole suite, no network); `[exceptions] no-ci`
  makes the local hooks the gate. `just check` adds audit/deny/machete (network).
- No fast-compile recipe on purpose (`docs/architecture.md`). Module layout: same file.

## Key design points

- `Command::exec` uses an `Rc<tempfile::TempFile>` shared with subprocess for stdout capture; the `Rc` is dropped (`drop(p)`) before reading so `Rc::get_mut` is always safe.
- `Context::new()` is infallible (uses `rustix::system::uname()`).
- Templates use minijinja v2 with auto-escape disabled (HTML escaping done explicitly with `| e`).
- The custom `rfc2822` filter converts ISO-8601 timestamps to RFC 2822 for display.
