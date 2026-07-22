.PHONY: help all clean test build release lint typecheck fmt check-fmt check-static-regexes markdownlint nixie

APP ?= mdtablefix
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
BUILD_JOBS ?=
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

check-static-regexes: ## Reject hand-rolled static regular expressions
	@status=0; \
	$(RG) -U --glob '*.rs' '\bstatic\b[^;=]*=\s*(?:[[:alnum:]_]+::)*LazyLock::new\s*\(\s*\|\|\s*(\{\s*)?(?:[[:alnum:]_]+::)*Regex::new' . || status=$$?; \
	case $$status in \
		0) echo "static regular expressions must use lazy_regex!"; exit 1 ;; \
		1) ;; \
		*) echo "failed to scan Rust sources (rg exit $$status)" >&2; exit $$status ;; \
	esac

markdownlint: ## Lint Markdown files
	$(MDLINT) "**/*.md"

nixie: ## Validate Mermaid diagrams
	nixie --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
