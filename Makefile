ci: fmt-check clippy test audit

check:
	cargo check --workspace --all-features --tests --examples

build:
	cargo build --workspace --all-features

test:
	cargo test --workspace --all-features --no-fail-fast

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

doc:
	cargo doc --all-features --no-deps

audit:
	cargo audit --deny-warnings

deny:
	cargo deny check

machete:
	cargo machete

clean:
	cargo clean

.PHONY: ci check build test clippy fmt fmt-check doc audit deny machete clean
