# Our apps, each one only when chosen: yami-files (folders, the file dialog of every app through the
# portal, «show in folder»), yami-view (pictures), yami-play (video and music). Their .desktop files
# and the defaults for their MIME types; unchecked over an earlier install, an app goes again with
# its defaults (mimeapps.list keeps every other entry), yami-files with its portal and D-Bus files.
# The binaries themselves come from build.sh.
# shellcheck shell=bash

plan_apps() {
  if on files; then add "$(t "файлы: yami-files и диалог выбора файлов" "files: yami-files and the file dialog" "arquivos: yami-files e diálogo de arquivos")" do_files
  elif app_here yami-files; then add "$(t "файлы: убрать yami-files (не выбран)" "files: remove yami-files (not chosen)" "arquivos: remover o yami-files (não escolhido)")" drop_files; fi
  if on view; then add "$(t "картинки: yami-view" "images: yami-view" "imagens: yami-view")" do_view
  elif app_here yami-view; then add "$(t "картинки: убрать yami-view (не выбран)" "images: remove yami-view (not chosen)" "imagens: remover o yami-view (não escolhido)")" drop_app yami-view; fi
  if on play; then add "$(t "плеер: yami-play" "player: yami-play" "player: yami-play")" do_play
  elif app_here yami-play; then add "$(t "плеер: убрать yami-play (не выбран)" "player: remove yami-play (not chosen)" "player: remover o yami-play (não escolhido)")" drop_app yami-play; fi
  return 0
}
app_here() { [[ -e $BIN/$1 || -e ${XDG_DATA_HOME:-$HOME/.local/share}/applications/$1.desktop ]]; }
# the .desktop file and the default for its MimeType list
app_desktop() {
  local data="${XDG_DATA_HOME:-$HOME/.local/share}"
  # на чистой системе ~/.config ещё нет, а xdg-mime его не создаёт
  mkdir -p "$CFG" "$data/applications"
  install -Dm644 "$ROOT/config/applications/$1.desktop" "$data/applications/$1.desktop" || return 1
  command -v xdg-mime >/dev/null || return 0
  # shellcheck disable=SC2046 # one argument per MIME type
  xdg-mime default "$1.desktop" $(sed -n 's/^MimeType=//p' "$ROOT/config/applications/$1.desktop" | tr ';' ' ')
}
# pictures open in yami-view, video and music in yami-play
do_view() { app_desktop yami-view; }
do_play() { app_desktop yami-play; }
# an app that is no longer chosen: its binary, .desktop and the defaults pointing at it
drop_app() {
  local data="${XDG_DATA_HOME:-$HOME/.local/share}"
  rm -f -- "$BIN/$1" "$data/applications/$1.desktop" "$data/icons/hicolor/scalable/apps/$1.svg"
  mime_forget "$1.desktop"
}
# mimeapps.list without the given .desktop (xdg-mime default wrote it); other entries stay as they are
mime_forget() {
  python3 -I - "$CFG/mimeapps.list" "$1" <<'PY'
import sys
from pathlib import Path
p, app = Path(sys.argv[1]), sys.argv[2]
try:
    lines = p.read_text().splitlines()
except OSError:
    sys.exit(0)
out = []
for l in lines:
    k, eq, v = l.partition("=")
    if eq and not l.startswith(("[", "#")) and app in v.split(";"):
        rest = [a for a in v.split(";") if a and a != app]
        if not rest:
            continue
        l = f"{k}={';'.join(rest)};"
    out.append(l)
if out != lines:
    p.write_text("\n".join(out) + "\n")
PY
}

# ── files: yami-files as the folder handler, the portal file dialog ─────────
do_files() {
  local data="${XDG_DATA_HOME:-$HOME/.local/share}"
  app_desktop yami-files || return 1
  # xdg-desktop-portal: FileChooser → yami-files (browsers, Telegram, GTK/KDE apps)
  install -Dm644 "$ROOT/config/portal/yami.portal" "$data/xdg-desktop-portal/portals/yami.portal"
  # which backend serves what: a niri-portals.conf of your own (no «# yami:» line) stays yours
  local pc="$CFG/xdg-desktop-portal/niri-portals.conf"
  if [[ ! -e $pc ]] || head -1 "$pc" | grep -q '^# yami:'; then install -Dm644 "$ROOT/config/portal/niri-portals.conf" "$pc"; fi
  mkdir -p "$data/dbus-1/services"
  sed "s|@BIN@|$BIN/yami-files|" "$ROOT/config/portal/org.freedesktop.impl.portal.desktop.yami.service" \
    >"$data/dbus-1/services/org.freedesktop.impl.portal.desktop.yami.service"
  # «показать в папке» из браузеров и прочих: org.freedesktop.FileManager1
  sed "s|@BIN@|$BIN/yami-files|" "$ROOT/config/portal/org.freedesktop.FileManager1.service" \
    >"$data/dbus-1/services/org.freedesktop.FileManager1.service"
  # Firefox and its forks pick the portal dialog only with this pref
  # profiles from profiles.ini too (they may live outside the browser folder)
  local prof
  while IFS= read -r prof; do
    [[ -f $prof/prefs.js ]] || continue
    grep -qs 'use-xdg-desktop-portal.file-picker' "$prof/user.js" ||
      printf '// yami: file dialogs through the portal (yami-files)\nuser_pref("widget.use-xdg-desktop-portal.file-picker", 1);\n' >>"$prof/user.js"
  done < <(python3 -I "$ROOT/tools/theme.py" --profiles 2>/dev/null)
  # the backend left from the previous build moves to the new binary as soon as no screencast or
  # file dialog is open (a running stream is not cut), and is started when it is not running:
  # xdg-desktop-portal takes ScreenCast's source types from the live backend, without it OBS and
  # browsers see nothing to capture. Only in a niri session or over a running backend: installed
  # from another desktop, it would take «show in folder» (FileManager1) there until logout
  if ((RUNNING_NIRI)) || pgrep -f '^[^ ]*yami-files --portal$' >/dev/null; then
    "$BIN/yami-files" --portal-reload 2>/dev/null
  fi
  # a first install: the portal frontend reads niri-portals.conf only when it starts
  if ((!UPDATE && SYSTEMD)) && systemctl --user is-active -q xdg-desktop-portal 2>/dev/null; then
    busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ReloadConfig >/dev/null 2>&1
    systemctl --user restart xdg-desktop-portal
  fi
  return 0
}
# yami-files unchecked: the backend stops, the portal and D-Bus files go (dialogs fall back to GTK),
# niri-portals.conf goes if it is ours
drop_files() {
  local data="${XDG_DATA_HOME:-$HOME/.local/share}" pc="$CFG/xdg-desktop-portal/niri-portals.conf"
  pkill -f "^$BIN/yami-files --portal" 2>/dev/null
  drop_app yami-files
  rm -f -- "$data/xdg-desktop-portal/portals/yami.portal" "$data/dbus-1/services/org.freedesktop.impl.portal.desktop.yami.service" \
    "$data/dbus-1/services/org.freedesktop.FileManager1.service"
  head -1 "$pc" 2>/dev/null | grep -q '^# yami:' && rm -f -- "$pc"
  if ((SYSTEMD)) && systemctl --user is-active -q xdg-desktop-portal 2>/dev/null; then
    busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ReloadConfig >/dev/null 2>&1
    systemctl --user restart xdg-desktop-portal
  fi
  return 0
}
