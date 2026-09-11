.PHONY: help all clean test build release lint typecheck fmt check-fmt check-ripgrep check-static-regexes markdownlint nixie mutants

APP ?= mdtablefix
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
BUILD_JOBS ?=
MUTANTS_JOBS ?= 3
# Where `cargo-mutants` builds. Per worktree, so two runs cannot collide.
MUTANTS_TMPDIR ?= $(HOME)/.cache/mdtablefix/mutants/$(notdir $(CURDIR))
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(HOME)/.bun/bin/markdownlint-cli2)
NIXIE ?= nixie
RG ?= rg

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

markdownlint: ## Lint Markdown files
	$(MDLINT) "**/*.md"

nixie: ## Validate Mermaid diagrams
	nixie --no-sandbox

mutants: ## Run mutation testing over the selection module
	@# Absolute, and outside the tree under test. Absolute because the tool's
	@# own child processes run inside the scratch copy of the tree, where a
	@# relative path does not exist. Outside because those children inherit
	@# TMPDIR, and a test suite whose temporary directories land inside this
	@# repository would fail the `--git` scenarios that assert on being outside
	@# one. `$HOME/.cache` is neither `/tmp`, which is not a build target here,
	@# nor inside the worktree.
	@mkdir -p $(MUTANTS_TMPDIR)
	TMPDIR=$(MUTANTS_TMPDIR) $(CARGO) mutants -j $(MUTANTS_JOBS)

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
