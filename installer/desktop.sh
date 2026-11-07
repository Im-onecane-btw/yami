# The steps that put the desktop into the home folder: the fonts, niri's config, the palette and what is
# made from it (only the programs chosen to restyle, restyle.sh), the app theme, then the install
# record, the counter and the shell restarted in place. One root step: the system services the
# desktop needs (bluetooth, power profiles, NetworkManager when nothing else runs the network).
# shellcheck shell=bash

plan_services() {
  on core && ((SUDO)) && add "$(t "службы: bluetooth, профили питания, сеть" "services: bluetooth, power profiles, network" "serviços: bluetooth, perfis de energia e rede")" do_services
  return 0
}
# the home folder: a first install writes the settings it was given, an update only regenerates
plan_desktop() {
  if ((UPDATE)); then
    add "$(t "шрифты" "fonts" "fontes")" do_fonts
    add "$(t "niri: конфиг" "niri: config" "niri: configuração")" do_niri_config
    add "$(t "палитра: всё, что из неё выводится" "palette: everything derived from it" "paleta: tudo deriva dela")" do_refresh
    return 0
  fi
  add "$(t "шрифты: Tiny5, DotGothic16, Terminess" "fonts: Tiny5, DotGothic16, Terminess" "fontes: Tiny5, DotGothic16 e Terminess")" do_fonts
  add "$(t "niri: конфиг" "niri: config" "niri: configuração")" do_niri_config
  add "$(t "палитра «$(pal_name "$pal")»: шелл, обои и выбранные программы" "palette «$(pal_name "$pal")»: the shell, wallpaper and the chosen programs" "paleta «$(pal_name "$pal")»: o shell, o papel de parede e os programas escolhidos")" do_dotfiles
  on look && add "$(t "тема программ: Qt, KDE, GTK" "app theme: Qt, KDE, GTK" "tema dos aplicativos: Qt, KDE e GTK")" do_look
  return 0
}
# last: what was installed, what the chosen components still miss (an update has no sudo: the
# shell offers one command for it), the counter, the running shell restarted
plan_record() {
  add "$(t "запись об установке (для yami update)" "install record (for yami update)" "registro de instalação (para yami update)")" do_stamp
  add "$(t "каких пакетов ещё не хватает" "packages still missing" "pacotes ainda ausentes")" do_need_pkgs
  add "$(t "счётчик установок (один раз, без данных)" "install counter (once, no data)" "contador de instalações (uma vez, sem dados)")" do_count
  ((RUNNING_NIRI)) && add "$(t "перезапуск шелла" "restart the shell" "reiniciar o shell")" do_restart_shell
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
  if ! t_on kitty && [[ $MONO != 1 ]]; then
    echo "Monocraft: kitty is not restyled, not downloaded"
  elif [[ -e $skip ]]; then
    rm -f -- "$mono"
    echo "Monocraft skipped (--no-monocraft): the terminal uses Terminess; --monocraft brings it back"
  elif [[ ! -s $mono ]]; then
    curl -fsSL --proto '=https' --retry 2 -o "$CACHE/monocraft.ttc" "$MONOCRAFT_URL" &&
      echo "$MONOCRAFT_SHA256  $CACHE/monocraft.ttc" | sha256sum -c --quiet - &&
      install -m644 "$CACHE/monocraft.ttc" "$mono" ||
      echo "Monocraft download failed or did not match its checksum, kitty falls back to Terminess"
    rm -f "$CACHE/monocraft.ttc"
  fi
  # crisp text in GTK and Qt apps (greyscale smoothing, pixel fonts at their own sizes): with them only
  if t_any gtk qt; then
    install -Dm644 "$ROOT/config/fontconfig/50-yami-pixel.conf" "$CFG/fontconfig/conf.d/50-yami-pixel.conf"
  else
    rm -f -- "$CFG/fontconfig/conf.d/50-yami-pixel.conf"
  fi
  fc-cache -f "$dir"
}

# ── niri config ─────────────────────────────────────────────────────────────
# the machine's own niri settings: nothing for now (the GPU needs no environment from us); config.kdl
# includes the file, so it is always there
write_local_kdl() {
  echo "// YAMI: written by install.sh for this machine. Re-run the installer to regenerate."
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
  mkdir -p "$STATE" "$HOME/Pictures/Screenshots"
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
  # what to restyle (themeApps, the cursor): theme.py below and every later run keep to it
  restyle_json || return 1
  # kitty: our file next to kitty.conf and one include line at its end; the rest of kitty.conf is yours
  if t_on kitty; then
    install -Dm644 "$ROOT/config/kitty/yami.conf" "$CFG/kitty/yami.conf"
    touch "$CFG/kitty/kitty.conf"
    grep -qx 'include yami.conf' "$CFG/kitty/kitty.conf" ||
      printf '\n# YAMI: settings → color → apps → kitty off takes this line out\ninclude yami.conf\n' >>"$CFG/kitty/kitty.conf"
  fi
  # wallpaper, niri borders and what is chosen to restyle (kitty, fastfetch, GTK, Qt, the cursor…)
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

# the first Qt/GTK setup, only for the ones chosen to restyle
do_look() { YAMI_RESTYLE="$(t_list)" "$ROOT/tools/apply-theme.sh"; }

# update: everything generated from the palette again (new theme.py outputs), settings untouched
do_refresh() {
  # an older settings file (appThemes) gets themeApps; a --restyle flag lands here too
  restyle_json || return 1
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
  curl -fsSL --max-time 15 -o /dev/null "$COUNT_URL" || { note "$(t "счётчик недоступен — не страшно" "counter unreachable, never mind" "contador indisponível, sem problema")"; return 0; }
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
