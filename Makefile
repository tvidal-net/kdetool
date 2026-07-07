# kwintool — common developer and install tasks.
#
#   make            build the release binary
#   make test       run the Rust unit tests and the KWin script harness
#   make lint       run clippy and check formatting
#   make install    install the CLI and the KWin script, then reload it
#   make reload     reload the resident KWin script (after editing main.js)

KWIN_PKG   := kwin
SCRIPT_ID  := KWinTool
SCRIPT_DIR := $(HOME)/.local/share/kwin/scripts/$(SCRIPT_ID)
MAIN_JS    := $(SCRIPT_DIR)/contents/code/main.js

SCRIPTING  := gdbus call --session --dest org.kde.KWin --object-path /Scripting \
              --method org.kde.kwin.Scripting

.PHONY: build test test-rust test-kwin lint fmt install reload uninstall clean

build:
	cargo build --release

test: test-rust test-kwin

test-rust:
	cargo test

test-kwin:
	node --test kwin/test/*.test.mjs

lint:
	cargo clippy --all-targets
	cargo fmt --check

fmt:
	cargo fmt

# Install the CLI to ~/.cargo/bin and (re)install the bundled KWin script.
install: build
	cargo install --path . --force
	@if kpackagetool6 --type KWin/Script --list 2>/dev/null | grep -qx $(SCRIPT_ID); then \
		kpackagetool6 --type KWin/Script --upgrade $(KWIN_PKG); \
	else \
		kpackagetool6 --type KWin/Script --install $(KWIN_PKG); \
	fi
	$(MAKE) reload

# Reload the resident script so an edited main.js takes effect without a logout.
reload:
	-$(SCRIPTING).unloadScript $(SCRIPT_ID)
	$(SCRIPTING).loadScript $(MAIN_JS) $(SCRIPT_ID)
	$(SCRIPTING).start

uninstall:
	-kpackagetool6 --type KWin/Script --remove $(KWIN_PKG)

clean:
	cargo clean
