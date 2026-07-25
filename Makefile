# kwintool — common developer and install tasks.
#
#   make            build the release binary
#   make test       run the Rust unit tests and the KWin script harness
#   make lint       run clippy and check formatting
#   make install    install the CLI, service and KWin script, then reload it
#   make reload     reload the resident KWin script (after editing main.js)

KWIN_PKG   := kwin
SCRIPT_ID  := KWinTool
SCRIPT_DIR := $(HOME)/.local/share/kwin/scripts/$(SCRIPT_ID)
MAIN_JS    := $(SCRIPT_DIR)/contents/code/main.js

BUS_NAME    := uk.tvidal.server
UNIT        := kwintool.service
DBUS_DIR    := $(HOME)/.local/share/dbus-1/services
SYSTEMD_DIR := $(HOME)/.config/systemd/user
# Where the installed binary actually lives (cargo may target ~/.local or
# ~/.cargo depending on config); the unit's ExecStart is rewritten to match.
KWINTOOL_BIN := $(shell command -v kwintool 2>/dev/null || printf '%s' "$(HOME)/.cargo/bin/kwintool")

SCRIPTING  := gdbus call --session --dest org.kde.KWin --object-path /Scripting \
              --method org.kde.kwin.Scripting

.PHONY: build test test-rust test-kwin lint fmt install install-service reload uninstall clean

build:
	cargo build --verbose --profile release

strip: build
	strip --verbose --strip-all target/release/kwintool

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
install: build strip install-service
	cargo install --path . --force
	@if kpackagetool6 --type KWin/Script --list 2>/dev/null | grep -qx $(SCRIPT_ID); then \
		kpackagetool6 --type KWin/Script --upgrade $(KWIN_PKG); \
	else \
		kpackagetool6 --type KWin/Script --install $(KWIN_PKG); \
	fi
	$(MAKE) reload

# Register the D-Bus-activated background service: drop the activation stub and
# the user unit where the session bus and systemd look, then reload both so the
# name `uk.tvidal.server` becomes activatable without a re-login.
install-service:
	install -d $(DBUS_DIR) $(SYSTEMD_DIR)
	install -m644 dbus/$(BUS_NAME).service $(DBUS_DIR)/$(BUS_NAME).service
	sed 's|^ExecStart=.*|ExecStart=$(KWINTOOL_BIN) --service|' \
		systemd/$(UNIT) > $(SYSTEMD_DIR)/$(UNIT)
	systemctl --user daemon-reload
	-dbus-send --session --type=method_call --dest=org.freedesktop.DBus \
		/org/freedesktop/DBus org.freedesktop.DBus.ReloadConfig

# Reload the resident script so an edited main.js takes effect without a logout.
reload:
	-$(SCRIPTING).unloadScript $(SCRIPT_ID)
	$(SCRIPTING).loadScript $(MAIN_JS) $(SCRIPT_ID)
	$(SCRIPTING).start

uninstall:
	-kpackagetool6 --type KWin/Script --remove $(KWIN_PKG)
	-rm -f $(DBUS_DIR)/$(BUS_NAME).service $(SYSTEMD_DIR)/$(UNIT)
	-systemctl --user daemon-reload

clean:
	cargo clean
