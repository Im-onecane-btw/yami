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
# alive = answers on its socket (not pgrep: `yami update apply` is a yami-shell process too)
# the new shell binds its socket anew: with an inode given, a ping counts only from another one
ino() { stat -c %i "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/yami-shell.sock" 2>/dev/null || echo none; }
up() {
  local i old=${1:-}
  for i in $(seq 1 80); do
    sleep 0.1
    [[ -n $old && $(ino) == "$old" ]] && continue
    "$BIN" msg ping >/dev/null 2>&1 && return 0
  done
  return 1
}
started=0
if "$BIN" msg ping >/dev/null 2>&1; then
  # exec of the new binary in place: same process, niri's autostart loop stays
  old="$(ino)"
  "$BIN" msg restart >/dev/null 2>&1 || true
  up "$old" && started=1
fi
if ((!started)) && [[ -n ${NIRI_SOCKET:-} ]]; then
  niri msg action spawn-sh -- "until $BIN; do sleep 1; done"
  up && started=1
fi
# the shell itself: yami-shell without arguments (CLI commands like `update apply` have them)
pid="$(pgrep -ax yami-shell | awk 'NF == 2 { print $1; exit }' || true)"
if ((started)) && [[ -n $pid ]]; then
  echo "deploy: yami-shell $("$BIN" --version | cut -d' ' -f2) running, pid $pid, $(($(awk '/VmRSS/{print $2}' "/proc/$pid/status") / 1024)) MB"
else
  echo "deploy: installed to $BIN (no niri session, not started)"
fi
