# The verb contract of usereport-rs (pdt-adlc ADR 0008).
#
# Migrated from a Makefile on 2026-08-18, by reduction: five of 17 targets are
# gone rather than ported — help (`just --list` builds the listing), all (a bare
# alias for build), clean (`cargo clean` is shorter than the recipe), and the two
# aggregates ci and pre-push, whose contents the contract already names: ci is
# `check`, pre-push is `adlc-verify`.
#
# The sixth removal is the point of the migration. `check` was
# `cargo check --workspace --all-features --tests --examples` — a compile, no
# codegen, no test run — and the ADLC's contract resolver preferred a target
# called `check`. So every attestation this repository ever produced proved that
# 99 test files COMPILE (pdt-adlc backlog I14). The name now means what the
# contract says it means, and the fast-compile verb is gone entirely: clippy
# compiles the same targets and checks more, so `cargo check` was never the
# cheaper answer to any question asked here.
#
# audit, deny and machete fetch an advisory database, so they hang off `check`
# and never off `adlc-verify` — repo-contract requirement 4.

cargo := "cargo"

default: adlc-verify

# --- the contract ------------------------------------------------------------

# What the ADLC gate runs: fmt-check, clippy, the whole suite. No network.
adlc-verify: lint test

# Everything, network included: the gate plus the dependency-hygiene pass.
check: lint test audit deny machete

# The whole suite — 385 tests, workspace-wide, all features, no fail-fast.
test:
    {{cargo}} test --workspace --all-features --no-fail-fast

# fmt-check + clippy.
lint: fmt-check clippy

# --- the individual checks ---------------------------------------------------

fmt-check:
    {{cargo}} fmt --check

clippy:
    {{cargo}} clippy --workspace --all-targets --all-features -- -D warnings

fmt:
    {{cargo}} fmt

# --- build -------------------------------------------------------------------

# Release binary, all features.
build:
    {{cargo}} build --all-features --release

doc:
    {{cargo}} doc --all-features --no-deps

# --- dependency hygiene (network: advisory database) -------------------------

audit:
    {{cargo}} audit --deny warnings

deny:
    {{cargo}} deny check

# Unused dependencies.
machete:
    {{cargo}} machete

# --- github ------------------------------------------------------------------

# GitHub Actions status (latest run per workflow).
workflows:
    #!/usr/bin/env bash
    set -euo pipefail
    export GH_PAGER=cat
    JQ='group_by(.workflowName) | map(max_by(.createdAt)) | sort_by(.workflowName) | .[] | [(.conclusion // .status), .workflowName, (.createdAt | fromdateiso8601 | strflocaltime("%d.%m.%Y %H:%M")), .displayTitle, .url] | @tsv'
    AWK='{ icon = "?"; col = "\033[37m"; if ($1 == "success") { icon = "✓"; col = "\033[32m" } else if ($1 == "failure") { icon = "✗"; col = "\033[31m" } else if ($1 == "cancelled") { icon = "⊘"; col = "\033[37m" } else if ($1 == "in_progress") { icon = "⏵"; col = "\033[33m" } else if ($1 == "queued") { icon = "⋯"; col = "\033[33m" } title = $4; if (length(title) > 40) title = substr(title, 1, 37) "..."; printf "  %s%s\033[0m %-24.24s  %s  %-40s  \033[2m%s\033[0m\n", col, icon, $2, $3, title, $5 }'
    gh run list -L 30 --json status,conclusion,workflowName,displayTitle,createdAt,url 2>/dev/null | jq -r "$JQ" | awk -F'\t' "$AWK"
