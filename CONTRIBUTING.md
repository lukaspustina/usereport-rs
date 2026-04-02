# Contributing

## Build

```sh
cargo build --all-features
```

## Test

```sh
cargo test --all-features
```

## Lint

```sh
cargo clippy --all-features
cargo fmt --check
```

## Run

```sh
cargo run --all-features -- --help
cargo run --all-features -- --output markdown
```

## Notes

- Pass `--all-features` for all commands; the binary and table output depend on the `bin` feature.
- Set `RUST_LOG=debug` to see debug output.
- Contributions welcome via pull requests. Please keep commits focused and tests green.
