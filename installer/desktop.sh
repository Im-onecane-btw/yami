# The steps that put the desktop into the home folder. yami-files for folders
# and file dialogs, the fonts, niri's config, the palette and everything made from it, the app theme,
# the system services, the install record and counter, and the shell restarted in place.
# shellcheck shell=bash

# ── files: yami-files as the folder handler, the portal file dialog ─────────
do_files() {
  local data="${XDG_DATA_HOME:-$HOME/.local/share}"
  # на чистой системе ~/.config ещё нет, а xdg-mime его не создаёт
  mkdir -p "$CFG" "$data/applications"
  install -Dm644 "$ROOT/config/applications/yami-files.desktop" "$data/applications/yami-files.desktop"
  install -Dm644 "$ROOT/config/applications/yami-view.desktop" "$data/applications/yami-view.desktop"
  install -Dm644 "$ROOT/config/applications/yami-play.desktop" "$data/applications/yami-play.desktop"
  if command -v xdg-mime >/dev/null; then
    xdg-mime default yami-files.desktop inode/directory
    # картинки — в yami-view, видео и музыка — в yami-play
    xdg-mime default yami-view.desktop $(sed -n 's/^MimeType=//p' "$ROOT/config/applications/yami-view.desktop" | tr ';' ' ')
    xdg-mime default yami-play.desktop $(sed -n 's/^MimeType=//p' "$ROOT/config/applications/yami-play.desktop" | tr ';' ' ')
  fi
  # xdg-desktop-portal: FileChooser → yami-files (browsers, Telegram, GTK/KDE apps)
  install -Dm644 "$ROOT/config/portal/yami.portal" "$data/xdg-desktop-portal/portals/yami.portal"
  install -Dm644 "$ROOT/config/portal/niri-portals.conf" "$CFG/xdg-desktop-portal/niri-portals.conf"
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

# ── fonts ───────────────────────────────────────────────────────────────────
do_fonts() {
  local dir="$HOME/.local/share/fonts/yami"
  install -Dm644 -t "$dir" "$ROOT"/fonts/*.ttf "$ROOT"/fonts/OFL-*.txt || return 1
  # Monocraft is 31 MB, the terminal's default font: --no-monocraft leaves it out (a marker keeps
  # updates from bringing it back), theme.py then picks Terminess
  local mono="$dir/Monocraft-nerd-fonts-patched.ttc" skip="$STATE/no-monocraft"
  if [[ $MONO == 0 ]]; then mkdir -p "$STATE" && : >"$skip"; elif [[ $MONO == 1 ]]; then rm -f -- "$skip"; fi
  if [[ -e $skip ]]; then
    rm -f -- "$mono"
    echo "Monocraft skipped (--no-monocraft): the terminal uses Terminess; --monocraft brings it back"
  elif [[ ! -s $mono ]]; then
    curl -fsSL --proto '=https' --retry 2 -o "$CACHE/monocraft.ttc" "$MONOCRAFT_URL" &&
      echo "$MONOCRAFT_SHA256  $CACHE/monocraft.ttc" | sha256sum -c --quiet - &&
      install -m644 "$CACHE/monocraft.ttc" "$mono" ||
      echo "Monocraft download failed or did not match its checksum, kitty falls back to Terminess"
    rm -f "$CACHE/monocraft.ttc"
  fi
  install -Dm644 "$ROOT/config/fontconfig/50-yami-pixel.conf" "$CFG/fontconfig/conf.d/50-yami-pixel.conf"
  fc-cache -f "$dir"
}

# ── niri config ─────────────────────────────────────────────────────────────
# the proprietary NVIDIA driver is loaded or installed (nouveau is not it)
nv_driver() { [[ -e /proc/driver/nvidia/version ]] || modinfo nvidia_drm >/dev/null 2>&1; }
write_local_kdl() {
  local env=()
  echo "// YAMI: written by install.sh for this machine. Re-run the installer to regenerate."
  # the polkit password window is yami-shell's own (no Qt/GTK agent)
  # the proprietary driver only (nouveau has no nvidia-drm GBM) and only when NVIDIA is the one GPU;
  # the __GL_ ones touch NVIDIA's own GL alone, so prime-run apps on a laptop keep them
  if on nvidia && nv_driver; then
    ((HYBRID)) || env+=('GBM_BACKEND "nvidia-drm"' '__GLX_VENDOR_LIBRARY_NAME "nvidia"' 'LIBVA_DRIVER_NAME "nvidia"' 'NVD_BACKEND "direct"')
    env+=('__GL_GSYNC_ALLOWED "1"' '__GL_VRR_ALLOWED "1"' '__GL_MaxFramesAllowed "1"' '__GL_SHADER_DISK_CACHE_SKIP_CLEANUP "1"')
  fi
  if ((${#env[@]})); then
    echo "environment {"
    printf '    %s\n' "${env[@]}"
    echo "}"
  fi
}
do_niri_config() {
  local n="$CFG/niri" old=""
  if [[ -d $n && ! -f $n/.yami ]]; then
    backup "$n"
    old="$n.bak-$STAMP/config.kdl"
  elif [[ -f $n/config.kdl ]]; then
    cp "$n/config.kdl" "$CACHE/config.kdl.prev"
    old="$CACHE/config.kdl.prev"
  fi
  mkdir -p "$n"
  [[ -f $n/user.kdl ]] || printf '%s\n' "// your tweaks on top of yami: binds, window rules, outputs. Included after the shell's" \
    "// files, so what you write here wins. Reference: https://github.com/YaLTeR/niri/wiki" >"$n/user.kdl"
  # keybindings come from the shell (yami-binds.kdl); carry `output` blocks of the previous
  # config into user.kdl
  install -m644 "$ROOT/config/niri/config.kdl" "$n/config.kdl" || return 1
  python3 -I - "$n/user.kdl" "$old" <<'PY' || return 1
import re, sys
from pathlib import Path
user, old = sys.argv[1:]
u = Path(user)
if old and Path(old).is_file() and "output " not in u.read_text():
    outs = re.findall(r'^output "[^"]*" \{.*?^\}', Path(old).read_text(), re.S | re.M)
    if outs:
        u.write_text(u.read_text().rstrip("\n") + "\n\n// outputs kept from the previous config\n" + "\n".join(outs) + "\n")
PY
  touch "$n/.yami"
  write_local_kdl >"$n/yami-local.kdl"
  # niri spawns `yami-shell` by name: make ~/.local/bin visible to the systemd session
  if ((SYSTEMD)); then
    mkdir -p "$CFG/environment.d"
    printf '# YAMI: the shell lives in ~/.local/bin\nPATH=${HOME}/.local/bin:${PATH}\n' >"$CFG/environment.d/60-yami-path.conf"
  fi
  if command -v niri >/dev/null && ! niri validate -c "$n/config.kdl"; then
    # this niri does not load the new config: the previous one goes back, so the next login keeps working
    if [[ $old == "$CACHE/config.kdl.prev" && -f $old ]]; then
      cp "$old" "$n/config.kdl" && echo "niri: the new config.kdl did not validate, the previous one is back"
    fi
    return 1
  fi
}

do_dotfiles() {
  mkdir -p "$STATE" "$HOME/Pictures/Screenshots" "$CFG/kitty" "$CFG/fastfetch"
  # shell settings: palette, language and, when chosen, the panel look
  local lk=""
  ((LOOK_SET)) && lk="${LOOK_GLASS[$look]} ${LOOK_LIFT[$look]} ${LOOK_INNER[$look]} ${LOOK_DIM[$look]}"
  local fe="" th=""
  ((FETCH_SET)) && fe="${FETCH_ID[$fetch]}"
  ((THEME_SET)) && th="${THEME_ID[$theme]}"
  python3 -I - "$CFG/yami-shell.json" "${PAL_ID[$pal]}" "$L" "$lk" "${FONT_ID[$font]}" "$fe" "$th" <<'PY' || return 1
import json, sys
from pathlib import Path
p = Path(sys.argv[1])
try:
    d = json.loads(p.read_text())
except (OSError, ValueError):
    d = {}
d["palette"], d["accent"], d["lang"] = sys.argv[2], "", sys.argv[3]
d["fontUi"] = "" if sys.argv[5] == "pixel" else "sans"
if sys.argv[6]:
    d["fetchFull"] = sys.argv[6] == "full"
if sys.argv[7]:
    d["light"] = sys.argv[7] == "light"
if sys.argv[4]:
    for k, v in zip(("glass", "lift", "inner", "dim"), sys.argv[4].split()):
        d[k] = int(v) / 100
p.write_text(json.dumps(d, ensure_ascii=False, indent=2) + "\n")
PY
  install -m644 "$ROOT/config/kitty/yami.conf" "$CFG/kitty/yami.conf"
  touch "$CFG/kitty/kitty.conf"
  grep -qx 'include yami.conf' "$CFG/kitty/kitty.conf" ||
    printf '\n# YAMI (remove this line to get the previous look back)\ninclude yami.conf\n' >>"$CFG/kitty/kitty.conf"
  if [[ -f "$CFG/fastfetch/config.jsonc" ]] && ! grep -qE "YAMI|DARK-4RMY" "$CFG/fastfetch/config.jsonc"; then
    cp "$CFG/fastfetch/config.jsonc" "$CFG/fastfetch/config.jsonc.bak-$STAMP"
  fi
  # wallpaper, kitty and fastfetch colors, niri borders, cursor, fonts
  local lt=()
  ((theme == 1)) && lt=(--light)
  python3 -I "$ROOT/tools/theme.py" --accent "#${PAL_HEX[$pal]}" "${lt[@]}"
  # a weak or virtual machine (old laptop, VM, no GPU acceleration) starts in lite mode: no blur,
  # no animations; settings → look turns it off. A file that already says `lite` is left alone
  "$BIN/yami-shell" config lite-auto
  # keybindings and the shell's window rules, before the first login (the shell keeps them current)
  "$BIN/yami-shell" binds write || return 1
  "$BIN/yami-shell" config kdl >"$CFG/niri/yami-fx.kdl" || return 1
}

do_look() { "$ROOT/tools/apply-theme.sh"; }

# update: everything generated from the palette again (new theme.py outputs), settings untouched
do_refresh() {
  python3 -I "$ROOT/tools/theme.py" || return 1
  "$BIN/yami-shell" binds write || return 1
  "$BIN/yami-shell" config kdl >"$CFG/niri/yami-fx.kdl"
}

# the install counter: one download of a tiny release file per machine, ever. GitHub counts
# downloads of release files; the request carries nothing about the machine (no id, no version, no
# data: what any download from GitHub sends) and the file is thrown away. --no-count,
# YAMI_NO_COUNT=1 or CI skip it. A marker in the state dir keeps it to one time per machine
COUNT_URL="https://github.com/Trendorin/yami/releases/latest/download/yami-installs.txt"
do_count() {
  local mark="$STATE/counted"
  ((COUNT)) && [[ -z ${YAMI_NO_COUNT:-} && -z ${CI:-} && ! -e $mark ]] || return 99
  command -v curl >/dev/null || return 99
  curl -fsSL --max-time 15 -o /dev/null "$COUNT_URL" || { note "$(t "счётчик недоступен — не страшно" "counter unreachable, never mind")"; return 0; }
  mkdir -p "${mark%/*}" && : >"$mark"
}
# what is installed and from where: `yami update` compares against it
do_stamp() {
  local c="" i
  # the choice, not what this run could do: an update without sudo skips root components, the
  # choice itself stays (a component added in this version comes in at its default)
  for i in "${!C_ID[@]}"; do ((C_WANT[i])) && c+="${C_ID[$i]} "; done
  "$BIN/yami-shell" update stamp "$ROOT" --components "${c% }" || return 1
  printf '%s\n' "${C_ID[*]}" >"$KNOWN_F"
}

# services the desktop needs; NetworkManager only when nothing else manages the network
do_services() {
  ((SYSTEMD)) || return 99
  local u
  for u in bluetooth power-profiles-daemon; do
    systemctl list-unit-files "$u.service" >/dev/null 2>&1 && $SU systemctl enable --now "$u.service" 2>/dev/null
  done
  if systemctl list-unit-files NetworkManager.service >/dev/null 2>&1 && ! systemctl is-enabled -q NetworkManager 2>/dev/null; then
    if systemctl is-active -q systemd-networkd || systemctl is-active -q connman || systemctl is-active -q netctl 2>/dev/null; then
      echo "another network manager is active: NetworkManager left disabled"
    else
      $SU systemctl enable --now NetworkManager.service
    fi
  fi
  return 0
}

# a game or any fullscreen window in focus: the restart would flash panels over it
game_focused() {
  local f
  f="$(niri msg --json focused-window 2>/dev/null)" || return 1
  grep -qiE '"app_id":"(steam_app_[0-9]+|gamescope|[^"]*\.exe)"' <<<"$f" || grep -q '"is_fullscreen":true' <<<"$f"
}
do_restart_shell() {
  if game_focused; then
    echo "a game has focus: the shell restarts at the next login (or: yami restart)"
    return 99
  fi
  pkill -x qs 2>/dev/null
  # the shell is the polkit agent now: a running Qt/GTK agent would keep the session's slot
  pkill -x polkit-kde-auth 2>/dev/null
  pkill -x polkit-gnome-au 2>/dev/null
  # alive = answers on its socket. Not pgrep: `yami update apply` runs as a process named
  # yami-shell too, and the shell would look alive while it is already gone
  if "$BIN/yami-shell" msg ping >/dev/null 2>&1; then
    # exec of the new binary in place: same process, niri's autostart loop stays; the new one
    # binds its socket anew, so wait for another inode before trusting a ping
    local ino
    ino="$(shell_sock_ino)"
    "$BIN/yami-shell" msg restart >/dev/null 2>&1
    shell_up "$ino" && return 0
  fi
  # no shell, or it did not come back: start one in niri's loop (a second copy exits by itself)
  niri msg action spawn-sh -- "until $BIN/yami-shell; do sleep 1; done" >/dev/null || return 1
  shell_up || { echo "the shell did not start: run yami run in a terminal to see why"; return 1; }
}
shell_sock_ino() { stat -c %i "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/yami-shell.sock" 2>/dev/null || echo none; }
# the shell answers on its socket within ~8 s (with an inode: a socket other than that one)
shell_up() {
  local i old=${1:-}
  for i in $(seq 1 80); do
    sleep 0.1
    [[ -n $old && $(shell_sock_ino) == "$old" ]] && continue
    "$BIN/yami-shell" msg ping >/dev/null 2>&1 && return 0
  done
  return 1
}
