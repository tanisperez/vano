.DEFAULT_GOAL := help

.PHONY: help run build release dist install install-latest uninstall check lint lint-fix format format-check test clean screenshots site

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "  run           Build and run the app (debug)"
	@echo "  build         Build in debug mode"
	@echo "  release       Build an optimized release binary"
	@echo "  dist          Build the release packages for this OS into target/dist"
	@echo "  install       Linux: build and install for this user under ~/.local (PREFIX=...)"
	@echo "  install-latest Linux: download the latest release and install it the same way"
	@echo "  uninstall     Linux: remove what make install or install-latest put under PREFIX"
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

# Linux only: the binary, the desktop entry and the icons, laid out like the tarball. The desktop
# entry is what gives the window its icon on Wayland (matched by the app id). Its Exec line gets
# the absolute path, so the menu entry works even when PREFIX/bin is not on the session's PATH.
PREFIX ?= $(HOME)/.local
APP_ID := codes.tanis.vano
# The same sizes as packaging/linux/package.sh.
ICON_SIZES := 16 24 32 48 64 128 256 512
ICONS_DIR = $(PREFIX)/share/icons/hicolor

install: release
	@[ "$$(uname -s)" = Linux ] || { echo "make install is for Linux, see packaging/ for the others"; exit 1; }
	install -Dm755 target/release/vano $(PREFIX)/bin/vano
	sed 's|^Exec=vano$$|Exec=$(PREFIX)/bin/vano|' packaging/linux/$(APP_ID).desktop \
		| install -Dm644 /dev/stdin $(PREFIX)/share/applications/$(APP_ID).desktop
	install -Dm644 packaging/icons/vano.svg $(ICONS_DIR)/scalable/apps/$(APP_ID).svg
	for size in $(ICON_SIZES); do \
		install -Dm644 packaging/icons/png/vano-$$size.png \
			$(ICONS_DIR)/$${size}x$${size}/apps/$(APP_ID).png; \
	done
	$(MAKE) --no-print-directory refresh-desktop-caches

# The latest published release instead of a local build, checked against its SHA256SUMS.
install-latest:
	packaging/linux/install-latest.sh $(PREFIX)
	$(MAKE) --no-print-directory refresh-desktop-caches

uninstall:
	rm -f $(PREFIX)/bin/vano $(PREFIX)/share/applications/$(APP_ID).desktop
	rm -f $(ICONS_DIR)/scalable/apps/$(APP_ID).svg \
		$(foreach size,$(ICON_SIZES),$(ICONS_DIR)/$(size)x$(size)/apps/$(APP_ID).png)
	$(MAKE) --no-print-directory refresh-desktop-caches

# Best effort: the tools may be missing, and an icon cache is only refreshed if there is one.
.PHONY: refresh-desktop-caches
refresh-desktop-caches:
	@if command -v update-desktop-database >/dev/null; then \
		update-desktop-database -q $(PREFIX)/share/applications || true; \
	fi
	@if [ -f $(ICONS_DIR)/icon-theme.cache ] && command -v gtk-update-icon-cache >/dev/null; then \
		gtk-update-icon-cache -q -t $(ICONS_DIR) || true; \
	fi

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
