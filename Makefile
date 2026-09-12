.PHONY: help all clean test build release lint typecheck fmt check-fmt check-ripgrep check-static-regexes check-verification-ledger check-prover-tools verus-install verus verus-selftest markdownlint nixie

APP ?= mdtablefix
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(HOME)/.bun/bin/markdownlint-cli2)
NIXIE ?= nixie
RG ?= rg
PROVER_TOOLS ?= uvx --from git+https://github.com/leynos/rust-prover-tools@$(shell cat tools/rust-prover-tools/REF) prover-tools
VERUS_RUN ?= env -u RUSTUP_TOOLCHAIN $(PROVER_TOOLS) verus run --repo-root .

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS)
	RUSTFLAGS="-D warnings" $(CARGO) test --doc --all-features $(BUILD_JOBS)

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: check-static-regexes check-verification-ledger ## Run Clippy with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)

typecheck: ## Type-check all targets and features
	$(CARGO) check --all-targets --all-features $(BUILD_JOBS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

check-ripgrep: ## Verify ripgrep is available
	@command -v "$(firstword $(RG))" >/dev/null 2>&1 || { \
		echo "ripgrep (rg) is required for static-regex linting" >&2; \
		exit 1; \
	}

check-static-regexes: check-ripgrep ## Reject hand-rolled static regular expressions
	@RG='$(RG)' scripts/check-static-regexes.sh .

check-verification-ledger: check-ripgrep ## Verify verification-ledger symbols exist
	@RG='$(RG)' scripts/check-verification-ledger.sh .

check-prover-tools: ## Verify the configured prover-tools runner is available
	@command -v "$(firstword $(PROVER_TOOLS))" >/dev/null 2>&1 || { \
		echo "prover-tools runner ($(firstword $(PROVER_TOOLS))) is required for Verus verification" >&2; \
		exit 1; \
	}

verus-install: check-prover-tools ## Install the pinned Verus release
	$(PROVER_TOOLS) verus install --repo-root .

verus: verus-install ## Verify the production-used Verus proof entry point
	$(VERUS_RUN) --proof-file verus/lib.rs

verus-selftest: verus-install ## Confirm Verus rejects the deliberately false smoke proof
	@output="$$(mktemp)"; \
	if $(VERUS_RUN) --proof-file verus/smoke.rs >"$$output" 2>&1; then \
		cat "$$output"; \
		rm -f "$$output"; \
		echo "Verus smoke proof unexpectedly succeeded" >&2; \
		exit 1; \
	fi; \
	if ! grep -Fq "Verus proofs failed" "$$output"; then \
		cat "$$output"; \
		rm -f "$$output"; \
		echo "Verus smoke proof did not reach the verifier" >&2; \
		exit 1; \
	fi; \
	cat "$$output"; \
	rm -f "$$output"

markdownlint: ## Lint Markdown files
	$(MDLINT) "**/*.md" "!**/.verus/**"

nixie: ## Validate Mermaid diagrams
	nixie --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
