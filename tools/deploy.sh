#!/usr/bin/env bash
# Build yami-shell and swap it into the running session: ~/.local/bin/yami-shell, then restart.
# Refuses while a game or any fullscreen window has focus (the restart flashes the bar and
# panels); --force skips that guard. --files-only installs yami-files and leaves the shell running.
# Usage: tools/deploy.sh [--force] [--files-only]
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$HOME/.local/bin/yami-shell"

if [[ ${1:-} != --force && ${1:-} != --files-only ]] && command -v niri >/dev/null && [[ -n ${NIRI_SOCKET:-} ]]; then
  focus="$(niri msg --json focused-window 2>/dev/null || true)"
  if grep -qiE '"app_id":"(steam_app_[0-9]+|gamescope|.*\.exe)"' <<<"$focus" ||
    grep -q '"is_fullscreen":true' <<<"$focus"; then
    echo "deploy: a game / fullscreen window has focus, not restarting the shell (use --force)" >&2
    exit 3
  fi
fi

cargo build --release --locked --workspace --manifest-path "$ROOT/shell/Cargo.toml"
install -Dm755 "$ROOT/shell/target/release/yami-files" "$HOME/.local/bin/yami-files"
install -Dm755 "$ROOT/shell/target/release/yami-view" "$HOME/.local/bin/yami-view"
install -Dm644 "$ROOT/config/applications/yami-files.desktop" "${XDG_DATA_HOME:-$HOME/.local/share}/applications/yami-files.desktop"
install -Dm644 "$ROOT/config/applications/yami-view.desktop" "${XDG_DATA_HOME:-$HOME/.local/share}/applications/yami-view.desktop"
# the portal backend runs from login: swap it for the new binary (not while a file dialog is
# open) and start it again at once. xdg-desktop-portal reads ScreenCast's source types from the
# running backend; while it is gone OBS and browsers see no screen to capture.
if ! pgrep -f '^[^ ]*yami-files --pick' >/dev/null && pkill -f '^[^ ]*yami-files --portal'; then
  sleep 0.3
  busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus StartServiceByName su \
    org.freedesktop.impl.portal.desktop.yami 0 >/dev/null 2>&1 || true
fi
if [[ ${1:-} == --files-only || ${2:-} == --files-only ]]; then
  echo "deploy: yami-files and yami-view installed"
  exit 0
fi
install -Dm755 "$ROOT/shell/target/release/yami-shell" "$BIN"
ln -sfn yami-shell "$HOME/.local/bin/yami"
# what runs now (`yami update` compares against it); a dirty tree is marked as such
"$BIN" update stamp "$ROOT" || true
if pgrep -x yami-shell >/dev/null; then
  "$BIN" msg quit || true
  sleep 1
fi
# niri's autostart loop restarts it only if the shell crashed; after a clean quit start a new loop
if ! pgrep -x yami-shell >/dev/null && [[ -n ${NIRI_SOCKET:-} ]]; then
  niri msg action spawn-sh -- "until $BIN; do sleep 1; done"
  sleep 1
fi
pid="$(pgrep -x yami-shell | head -1 || true)"
if [[ -n $pid ]]; then
  echo "deploy: yami-shell $("$BIN" --version | cut -d' ' -f2) running, pid $pid, $(($(awk '/VmRSS/{print $2}' "/proc/$pid/status") / 1024)) MB"
else
  echo "deploy: installed to $BIN (no niri session, not started)"
fi
