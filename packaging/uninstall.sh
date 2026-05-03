#!/usr/bin/env bash
# Reverse what install.sh did. Run interactively from the workspace root.
set -euo pipefail

BIN_DIR="$HOME/.local/bin"
DATA_DIR="$HOME/.local/share/audio-monitor"
SYSTEMD_DIR="$HOME/.config/systemd/user"
AUTOSTART_DIR="$HOME/.config/autostart"

systemctl --user disable --now audio-monitord.service 2>/dev/null || true

if pgrep -f "$BIN_DIR/audio-monitor-tray$" >/dev/null; then
    pkill -f "$BIN_DIR/audio-monitor-tray$" || true
fi

rm -f "$BIN_DIR/audio-monitord"
rm -f "$BIN_DIR/audio-monitor-tray"
rm -f "$DATA_DIR/speaker.png"
rmdir --ignore-fail-on-non-empty "$DATA_DIR" 2>/dev/null || true
rm -f "$SYSTEMD_DIR/audio-monitord.service"
rm -f "$AUTOSTART_DIR/audio-monitor-tray.desktop"
systemctl --user daemon-reload

echo "✓ Uninstalled."
