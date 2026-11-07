# Moving installs of earlier versions forward. The old name (DARK-4RMY: its
# settings, state and clipboard pins come along, its files go) and the leftovers of old releases.
# Every step here is «not needed» on a machine that never had them.
# shellcheck shell=bash

# leftovers of earlier versions, removed once: the old fork (dark-niri on angel-niri) with its
# drop-in, a font never used (Handjet), the blur rule that covered whole shell surfaces (the shell
# now sends exact blur regions and rewrites yami-fx.kdl itself)
do_legacy() {
  local d="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/niri.service.d/dark-niri.conf" f="$HOME/.local/share/fonts/yami" any=0
  if [[ -e $d || -d $HOME/.local/lib/dark-niri || -d $HOME/.local/share/yami/fork ]]; then
    rm -f -- "$d"
    [[ "$(readlink "$BIN/niri" 2>/dev/null)" == "$HOME/.local/lib/dark-niri"/* ]] && rm -f -- "$BIN/niri"
    rm -rf -- "$HOME/.local/lib/dark-niri" "$HOME/.local/share/yami/fork"
    ((SYSTEMD)) && systemctl --user daemon-reload 2>/dev/null
    any=1
  fi
  [[ -e $f/Handjet.ttf || -e $f/OFL-handjet.txt ]] && { rm -f -- "$f/Handjet.ttf" "$f/OFL-handjet.txt"; any=1; }
  grep -qs 'yami-(bar' "$CFG/niri/yami-fx.kdl" && { rm -f -- "$CFG/niri/yami-fx.kdl"; any=1; }
  ((any)) || return 99
}

# ── the old name (DARK-4RMY) ──────────────────────────────────────────────────
old_install() { [[ -e $BIN/dark4rmy-shell || -e $CFG/dark4rmy-shell.json || -e $CFG/niri/.dark4rmy || -d $HOME/.local/share/fonts/dark4rmy ]]; }
# early: copy settings, state and clipboard pins, so the steps below find them under the new name
do_migrate() {
  local pair a b
  for pair in "$CFG/dark4rmy-shell.json|$CFG/yami-shell.json" \
    "$HOME/.local/state/dark4rmy|$STATE" \
    "$HOME/.local/share/dark4rmy-clip|$HOME/.local/share/yami-clip"; do
    a="${pair%%|*}" b="${pair#*|}"
    [[ -e $a && ! -e $b ]] && cp -a -- "$a" "$b"
  done
  [[ -f $CFG/niri/.dark4rmy ]] && touch "$CFG/niri/.yami"
  while IFS= read -r a; do
    b="${a%.bak-dark4rmy}.bak-yami"
    [[ -e $b ]] || mv -- "$a" "$b"
  done < <(find "$CFG" -maxdepth 3 -name '*.bak-dark4rmy' 2>/dev/null)
  return 0
}
# late, once the new config is in place: stop the old shell and remove its files
do_drop_old() {
  if pgrep -x dark4rmy-shell >/dev/null; then
    "$BIN/dark4rmy-shell" msg quit 2>/dev/null
    sleep 1
  fi
  rm -f -- "$BIN/dark4rmy-shell" "$CFG/dark4rmy-shell.json" "$CFG/niri/.dark4rmy" \
    "$CFG/niri/dark4rmy-local.kdl" "$CFG/niri/dark4rmy-colors.kdl" "$CFG/niri/dark4rmy-fx.kdl" \
    "$CFG/environment.d/60-dark4rmy-path.conf" "$CFG/fontconfig/conf.d/50-dark4rmy-pixel.conf" \
    "$CFG/kitty/dark4rmy.conf" "$CFG/kitty/dark4rmy-colors.conf" "$CFG/fastfetch/dark4rmy-logo.txt" \
    "$HOME/.local/share/color-schemes/DARK4RMY.colors"
  # qt6ct switches to the new files only with the app theme step
  grep -qs dark4rmy "$CFG/qt6ct/qt6ct.conf" || rm -f -- "$CFG/qt6ct/colors/dark4rmy.conf" "$CFG/qt6ct/qss/dark4rmy.qss"
  rm -rf -- "$HOME/.local/state/dark4rmy" "$HOME/.local/share/dark4rmy-clip" "$HOME/.local/share/dark4rmy-dots"
  # a running kitty keeps the old font paths and dies when it reloads a face from a deleted
  # file; the same fonts are already in fonts/yami, the old copy goes on the next run
  if pgrep -x kitty >/dev/null; then
    echo "kitty is running: ~/.local/share/fonts/dark4rmy is kept until the next run"
  else
    rm -rf -- "$HOME/.local/share/fonts/dark4rmy"
  fi
  [[ -f $CFG/kitty/kitty.conf ]] && sed -i '/^# DARK-4RMY/d; /^include dark4rmy\.conf$/d' "$CFG/kitty/kitty.conf"
  fc-cache -f >/dev/null 2>&1
  return 0
}
