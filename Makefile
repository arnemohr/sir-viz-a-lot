.DEFAULT_GOAL := help

# `make help` walks this file's `## ` markers; keep them on the same line as
# the target declaration.

.PHONY: help setup hooks build build-release build-show run watch test test-cargo test-gpu \
        lint fmt fmt-check check ci bundle clean

help: ## Show available targets
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

setup: hooks ## Install Rust + cargo subcommand tools via mise; enable git hooks
	mise install

hooks: ## Point git at .githooks/ (idempotent; called by `setup`)
	git config core.hooksPath .githooks

build: ## Compile the debug binary
	cargo build

build-release: ## Compile the standard release binary
	cargo build --release

build-show: ## Compile the show-day binary (LTO, single codegen unit, panic=abort)
	cargo build --profile release-show

run: ## Run the debug binary
	cargo run

watch: ## Hot-restart on file save (requires cargo-watch)
	cargo watch -x run

test: ## Run unit + integration tests via cargo-nextest (faster, terser output)
	cargo nextest run

test-cargo: ## Vanilla `cargo test` (use if cargo-nextest is unavailable, or for doctests)
	cargo test

test-gpu: ## Run unit + headless-wgpu golden-image tests
	cargo nextest run --features gpu-tests

lint: ## Run clippy with warnings as errors
	cargo clippy --all-targets --all-features -- -D warnings

fmt: ## Format the workspace
	cargo fmt --all

fmt-check: ## Verify formatting (used in CI)
	cargo fmt --all --check

check: ## Type-check without producing artifacts
	cargo check --all-targets

ci: fmt-check lint test ## Run everything CI runs (incl. doctests; nextest skips them)
	cargo test --doc --quiet

bundle: build-show ## Build a macOS .app bundle (requires cargo-bundle)
	cargo bundle --profile release-show

clean: ## Remove target/
	cargo clean
