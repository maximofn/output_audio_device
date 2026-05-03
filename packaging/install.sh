#!/usr/bin/env bash
# Install the Rust audio-monitord daemon + audio-monitor-tray frontend
# into the current user's ~/.local hierarchy and wire them up so the
# daemon runs as a systemd --user service and the tray autostarts on
# login. Run from the workspace root after `cargo build --release`.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BIN_DIR="$HOME/.local/bin"
DATA_DIR="$HOME/.local/share/audio-monitor"
SYSTEMD_DIR="$HOME/.config/systemd/user"
AUTOSTART_DIR="$HOME/.config/autostart"

DAEMON_BIN="$REPO_ROOT/target/release/audio-monitord"
TRAY_BIN="$REPO_ROOT/target/release/audio-monitor-tray"

if [[ ! -x "$DAEMON_BIN" || ! -x "$TRAY_BIN" ]]; then
    echo "ERROR: build the binaries first:" >&2
    echo "  cargo build --release --workspace" >&2
    exit 1
fi

mkdir -p "$BIN_DIR" "$DATA_DIR" "$SYSTEMD_DIR" "$AUTOSTART_DIR"

install -m 0755 "$DAEMON_BIN" "$BIN_DIR/audio-monitord"
install -m 0755 "$TRAY_BIN"   "$BIN_DIR/audio-monitor-tray"
install -m 0644 "$REPO_ROOT/assets/speaker.png" "$DATA_DIR/speaker.png"
install -m 0644 "$SCRIPT_DIR/systemd/audio-monitord.service" "$SYSTEMD_DIR/audio-monitord.service"
install -m 0644 "$SCRIPT_DIR/autostart/audio-monitor-tray.desktop" "$AUTOSTART_DIR/audio-monitor-tray.desktop"

systemctl --user daemon-reload
systemctl --user enable --now audio-monitord.service

# The tray is a .desktop autostart entry, NOT a systemd unit (it needs the
# graphical session DBus). Launch it now so the user does not have to log
# out before seeing the icon.
if pgrep -f "$BIN_DIR/audio-monitor-tray$" >/dev/null; then
    pkill -f "$BIN_DIR/audio-monitor-tray$" || true
    sleep 0.3
fi
nohup "$BIN_DIR/audio-monitor-tray" >/dev/null 2>&1 & disown

cat <<EOF

✓ Installed binaries to: $BIN_DIR
✓ Installed icon to:     $DATA_DIR/speaker.png
✓ Daemon service:        systemctl --user status audio-monitord
✓ Tray autostarts via:   $AUTOSTART_DIR/audio-monitor-tray.desktop

To uninstall: ./packaging/uninstall.sh
EOF
