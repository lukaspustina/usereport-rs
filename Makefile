# usereport-rs — Top-level Makefile

SHELL         := /bin/bash
.DEFAULT_GOAL := help

# ── Project metadata ─────────────────────────────────────────────
APP     := usereport
VERSION := $(shell grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/' 2>/dev/null || echo "unknown")
GIT_SHA := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")

# ── Tools ────────────────────────────────────────────────────────
CARGO := cargo

# ── Flags (override from CLI: make CARGO_FLAGS=--release build) ──
CARGO_FLAGS ?=

# ── Phony targets ────────────────────────────────────────────────
.PHONY: all build check test lint ci pre-push clean \
        fmt fmt-check clippy doc \
        audit deny machete \
        workflows \
        help

# ══════════════════════════════════════════════════════════════════
#  Help
# ══════════════════════════════════════════════════════════════════

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | \
		awk -F ':.*## ' '{printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' | sort

# ══════════════════════════════════════════════════════════════════
#  Build
# ══════════════════════════════════════════════════════════════════

all: build ## Build release binary

build: ## Build release binary
	$(CARGO) build --all-features --release $(CARGO_FLAGS)

check: ## Fast compile check (cargo check)
	$(CARGO) check --workspace --all-features --tests --examples $(CARGO_FLAGS)

# ══════════════════════════════════════════════════════════════════
#  Test & lint
# ══════════════════════════════════════════════════════════════════

test: ## Run all tests
	$(CARGO) test --workspace --all-features --no-fail-fast $(CARGO_FLAGS)

clippy: ## Run clippy with -D warnings
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

fmt: ## Format code
	$(CARGO) fmt

fmt-check: ## Check formatting
	$(CARGO) fmt --check

doc: ## Build documentation
	$(CARGO) doc --all-features --no-deps

lint: fmt-check clippy ## Run all lints (fmt-check + clippy)

# ══════════════════════════════════════════════════════════════════
#  Security & dependency hygiene
# ══════════════════════════════════════════════════════════════════

audit: ## Run cargo audit
	$(CARGO) audit --deny warnings

deny: ## Run cargo deny
	$(CARGO) deny check

machete: ## Check for unused dependencies
	$(CARGO) machete

# ══════════════════════════════════════════════════════════════════
#  GitHub
# ══════════════════════════════════════════════════════════════════

workflows: ## GitHub Actions status (latest run per workflow)
	@export GH_PAGER=cat; \
	JQ='group_by(.workflowName) | map(max_by(.createdAt)) | sort_by(.workflowName) | .[] | [(.conclusion // .status), .workflowName, (.createdAt | fromdateiso8601 | strflocaltime("%d.%m.%Y %H:%M")), .displayTitle, .url] | @tsv'; \
	AWK='{ icon = "?"; col = "\033[37m"; if ($$1 == "success") { icon = "✓"; col = "\033[32m" } else if ($$1 == "failure") { icon = "✗"; col = "\033[31m" } else if ($$1 == "cancelled") { icon = "⊘"; col = "\033[37m" } else if ($$1 == "in_progress") { icon = "⏵"; col = "\033[33m" } else if ($$1 == "queued") { icon = "⋯"; col = "\033[33m" } title = $$4; if (length(title) > 40) title = substr(title, 1, 37) "..."; printf "  %s%s\033[0m %-24.24s  %s  %-40s  \033[2m%s\033[0m\n", col, icon, $$2, $$3, title, $$5 }'; \
	gh run list -L 30 --json status,conclusion,workflowName,displayTitle,createdAt,url 2>/dev/null | jq -r "$$JQ" | awk -F'\t' "$$AWK"

# ══════════════════════════════════════════════════════════════════
#  CI pipelines
# ══════════════════════════════════════════════════════════════════

ci: lint test audit deny machete ## Full CI pipeline (lint + test + audit)

pre-push: fmt-check clippy test ## Checks to run before pushing
	@echo ""
	@echo "All checks passed. Safe to push."

# ══════════════════════════════════════════════════════════════════
#  Housekeeping
# ══════════════════════════════════════════════════════════════════

clean: ## Remove build artefacts (cargo clean)
	$(CARGO) clean
