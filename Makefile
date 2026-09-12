.PHONY: help all clean test build release lint typecheck fmt check-fmt check-ripgrep check-static-regexes check-prover-tools check-verus-z3 install-verus verus markdownlint nixie

APP ?= mdtablefix
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(HOME)/.bun/bin/markdownlint-cli2)
NIXIE ?= nixie
RG ?= rg
PROVER_TOOLS ?= prover-tools
VERUS_TOOLCHAIN ?= 1.98.0
VERUS_INSTALL_FLAGS ?= --repo-root .
VERUS_BIN ?= .verus/0.2026.09.06.8dea4a2/verus/verus
VERUS_PROOF ?= verus/process_buffer.rs
VERUS_Z3_PATH ?=

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

lint: check-static-regexes ## Run Clippy with warnings denied
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

check-prover-tools: ## Verify rust-prover-tools is available for Verus proofs
	@command -v "$(firstword $(PROVER_TOOLS))" >/dev/null 2>&1 || { \
		echo "rust-prover-tools (prover-tools) is required for Verus proofs" >&2; \
		exit 1; \
	}

check-verus-z3: ## Verify an explicitly pinned Z3 executable is available
	@test -n "$(VERUS_Z3_PATH)" && test -x "$(VERUS_Z3_PATH)" || { \
		echo "set VERUS_Z3_PATH to a Z3 4.16.0 executable for Verus proofs" >&2; \
		exit 1; \
	}

install-verus: check-prover-tools ## Install the checksum-pinned Verus release
	$(PROVER_TOOLS) verus install $(VERUS_INSTALL_FLAGS)

verus: install-verus check-verus-z3 ## Verify the ProcessBuffer chunk ledger
	RUSTUP_TOOLCHAIN=$(VERUS_TOOLCHAIN) VERUS_Z3_PATH="$(VERUS_Z3_PATH)" $(VERUS_BIN) $(VERUS_PROOF)

markdownlint: ## Lint Markdown files
	$(MDLINT) "**/*.md"

nixie: ## Validate Mermaid diagrams
	nixie --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
