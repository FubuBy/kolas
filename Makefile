.PHONY: help fmt fmt-check clippy build test check

.DEFAULT_GOAL := help

help: ## Lists all available commands with their descriptions.
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk -F ':.*##' '{printf "  %-12s %s\n", $$1, $$2}'

fmt: ## Formats the codebase in place using rustfmt.
	cargo fmt

fmt-check: ## Checks that the codebase is formatted, without modifying any files (CI-friendly).
	cargo fmt -- --check

clippy: ## Lints all targets and features with clippy, treating warnings as errors.
	cargo clippy --all-targets --all-features -- -D warnings

build: ## Builds the project.
	cargo build

test: ## Runs the test suite (feature and unit integration tests).
	cargo test

check: fmt-check clippy build test ## Runs all required checks in order: format check, lint, build, tests.
