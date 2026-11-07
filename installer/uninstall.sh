# ./install.sh --uninstall. The shell, its apps and files go, backups come
# back, the login screen goes (login.sh), the settings file stays.

uninstall() {
  printf '\e[H\e[J'
  banner 1
  say "$(t "удаляю YAMI" "removing YAMI")"
  ((DRY)) && { note "--dry-run"; return; }
  pgrep -x yami-shell >/dev/null && "$BIN/yami-shell" msg quit 2>/dev/null
  [[ -x $HOME/.local/lib/yami-niri/yami-niri.sh ]] && "$HOME/.local/lib/yami-niri/yami-niri.sh" off
  rm -rf "$HOME/.local/lib/yami-niri" "$HOME/.local/lib/yami"
  rm -f "$BIN/yami-shell" "$BIN/yami" "$STATE/install.json" "$STATE/no-monocraft" "$STATE/update.json" "$STATE/update-keys" "$CFG/environment.d/60-yami-path.conf" "$CFG/fontconfig/conf.d/50-yami-pixel.conf" \
    "$CFG/niri/yami-binds.kdl" "$CFG/niri/yami-fx.kdl"
  local data="${XDG_DATA_HOME:-$HOME/.local/share}"
  pkill -f "$BIN/yami-files --portal" 2>/dev/null
  rm -f "$BIN/yami-files" "$BIN/yami-view" "$BIN/yami-play" "$data/applications/yami-files.desktop" "$data/applications/yami-view.desktop" "$data/applications/yami-play.desktop" "$data/xdg-desktop-portal/portals/yami.portal" \
    "$data/dbus-1/services/org.freedesktop.impl.portal.desktop.yami.service" "$data/dbus-1/services/org.freedesktop.FileManager1.service" \
    "$CFG/xdg-desktop-portal/niri-portals.conf" "$CFG/niri/yami-cursor.kdl" "$CFG/btop/themes/yami.theme"
  # pixel cursors: yami-<accent>, and yami-<accent>-l for the light theme
  find "$data/icons" -maxdepth 1 -type d -regextype posix-extended -regex '.*/yami-[0-9a-f]{6}(-l)?' -exec rm -rf {} + 2>/dev/null
  # the MangoHud config theme.py wrote (one without its mark is the user's own)
  head -1 "$CFG/MangoHud/MangoHud.conf" 2>/dev/null | grep -q '^# yami: written by tools/theme.py' && rm -f "$CFG/MangoHud/MangoHud.conf"
  rm -rf "$HOME/.local/share/fonts/yami" "$HOME/.local/share/yami" "$CFG/qt6ct/qss/yami"
  uninstall_login
  rm -f "$STATE/packages-needed" "$STATE/components-known" "$STATE/settings-newer.json"
  # the build cache (gigabytes); yami-niri sources only without patch work in them
  rm -rf -- "$CACHE/target" "$CACHE/sddm-yami" "$CACHE/yami-cursor" "$CACHE/update.log" "$CACHE/update.lock"
  local src="${YAMI_NIRI_SRC:-$CACHE/niri-src}"
  if niri_src_plain "$src"; then
    rm -rf -- "$src"
  elif [[ -d $src ]]; then
    note "$(t "в $src твоя работа над патчами — оставил" "your patch work is in $src: kept")"
  fi
  rm -f "$CFG/qt6ct/qss/yami.qss" "$CFG/qt6ct/colors/yami.conf"
  fc-cache -f >/dev/null 2>&1
  if [[ -f "$CFG/kitty/kitty.conf" ]]; then
    sed -i '/^# YAMI/d; /^include yami.conf$/d' "$CFG/kitty/kitty.conf"
    rm -f "$CFG/kitty/yami.conf"
  fi
  local b
  b="$(ls -d "$CFG"/niri.bak-* 2>/dev/null | sort | tail -n1)"
  if [[ -n $b ]]; then
    mv "$CFG/niri" "$CFG/niri.yami-$STAMP" && mv "$b" "$CFG/niri"
    say "$(t "niri: вернул" "niri: restored") $b"
  else
    note "$(t "niri: прежнего конфига нет, оставил текущий" "niri: no previous config, kept the current one")"
  fi
  while IFS= read -r b; do mv -f "$b" "${b%.bak-yami}"; done < <(find "$CFG" -maxdepth 3 -name '*.bak-yami' 2>/dev/null)
  say "$(t "готово. настройки шелла остались в" "done. shell settings are kept in") $CFG/yami-shell.json"
  exit 0
}
