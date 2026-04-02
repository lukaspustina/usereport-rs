check:
	cargo check --workspace --all-features --tests --examples

build:
	cargo build --workspace --all-features

test:
	cargo test --workspace --all-features

clean-package:
	cargo clean -p $$(cargo read-manifest | jq -r .name)

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

audit:
	cargo audit --deny-warnings

release: release-bump build
	git commit -am "Bump to version $$(cargo read-manifest | jq .version)"
	git tag v$$(cargo read-manifest | jq -r .version)

release-bump:
	cargo bump

publish:
	git push && git push --tags


.PHONY: check build test clean-package clippy fmt fmt-check audit release release-bump publish
