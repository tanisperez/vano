.DEFAULT_GOAL := help

.PHONY: help run build release dist check lint lint-fix format format-check test clean screenshots site

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "  run           Build and run the app (debug)"
	@echo "  build         Build in debug mode"
	@echo "  release       Build an optimized release binary"
	@echo "  dist          Build the release packages for this OS into target/dist"
	@echo "  check         Fast type-check without producing a binary"
	@echo "  lint          Run Clippy (warnings are errors)"
	@echo "  lint-fix      Run Clippy and apply automatic fixes"
	@echo "  format        Format all Rust code with rustfmt"
	@echo "  format-check  Check formatting without modifying files"
	@echo "  test          Run all tests"
	@echo "  clean         Remove build artifacts"
	@echo "  screenshots   Regenerate the website screenshots (site/img)"
	@echo "  site          Serve the website on http://localhost:8000"

run:
	cargo run

build:
	cargo build

release:
	cargo build --release

# The workspace version from Cargo.toml, for the package names.
VERSION := $(shell sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)

# Linux needs nfpm on PATH, macOS the Xcode command line tools. On Windows, compile
# packaging/windows/vano.iss with Inno Setup (see its README).
dist: release
	@case "$$(uname -s)" in \
	Linux) packaging/linux/package.sh $(VERSION) target/release/vano target/dist ;; \
	Darwin) packaging/macos/bundle.sh $(VERSION) target/release/vano target/dist ;; \
	*) echo "make dist supports Linux and macOS; see packaging/windows/README.md"; exit 1 ;; \
	esac

check:
	cargo check --all-targets

lint:
	cargo clippy --all-targets -- -D warnings

lint-fix:
	cargo clippy --all-targets --fix --allow-dirty --allow-staged

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

test:
	cargo test --all-targets

clean:
	cargo clean

screenshots:
	site/screenshots/capture.sh

site:
	python3 -m http.server -d site 8000
