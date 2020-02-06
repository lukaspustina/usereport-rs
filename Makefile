check:
	cargo check --workspace --all-features --tests --examples --benches

build:
	cargo build --workspace --all-features --tests --examples --benches

test:
	cargo test --workspace --all-features

clean-package:
	cargo clean -p $$(cargo read-manifest | jq -r .name)

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings $$(source ".clippy.args")

fmt:
	cargo +nightly fmt

audit:
	cargo audit --deny-warnings

release: release-bump all
	git commit -am "Bump to version $$(cargo read-manifest | jq .version)"
	git tag v$$(cargo read-manifest | jq -r .version)

release-bump:
	cargo bump

publish:
	git push && git push --tags


.PHONY:

