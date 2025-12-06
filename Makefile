.PHONY: help build test lint fmt clean doc check run-example shell-completions

help:
	@echo "sqllog2db Development Tasks"
	@echo "============================"
	@echo ""
	@echo "build              Build the project in debug mode"
	@echo "release            Build the project in release mode"
	@echo "test               Run all tests"
	@echo "lint               Run clippy linter"
	@echo "fmt                Format code with rustfmt"
	@echo "fmt-check          Check code formatting without changes"
	@echo "doc                Generate and open documentation"
	@echo "check              Run cargo check"
	@echo "clean              Clean build artifacts"
	@echo "run-example        Run with example config (CSV)"
	@echo "shell-completions  Generate shell completion scripts"
	@echo "all                Run all checks (lint, fmt-check, test)"

build:
	cargo build

release:
	cargo build --release

test:
	cargo test --lib --all-features

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

doc:
	cargo doc --no-deps --all-features --open

check:
	cargo check --all-features

clean:
	cargo clean
	rm -rf target/

run-example:
	@echo "Generating config..."
	cargo run -- init --force
	@echo "Running with generated config..."
	cargo run -- run --config config.toml

shell-completions:
	@echo "Generating shell completions..."
	cargo build --release
	@mkdir -p completions
	./target/release/sqllog2db completions bash > completions/sqllog2db.bash
	./target/release/sqllog2db completions zsh > completions/_sqllog2db
	./target/release/sqllog2db completions fish > completions/sqllog2db.fish
	@echo "Shell completions generated in completions/"

all: lint fmt-check test
	@echo "✓ All checks passed!"
