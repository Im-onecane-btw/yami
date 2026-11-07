# What can be installed. core and apps always; tools, games, the app theme,
# the login screen, the NVIDIA fixes and yami-niri by what this machine is (detect.sh) and the flags.
# An update repeats the choice of the first install (install.json, written by `yami update stamp`).

C_ID=(core apps tools games look login nvidia fork)
# components this installer offered last time: one that came later (games, tools) starts at its
# default on an update instead of «off because it was not in the list»
KNOWN_F="$STATE/components-known"
# what was installed (the shell's `update stamp`): an update repeats the same choice
INSTALLED="$STATE/install.json"

components_init() {
  C_ON=(1 1 "$TOOLS_FLAG" "$GAMES_ON" 1 "$LOGIN_ON" "$NVIDIA" "$FORK_ON")
  COMP=""
  local known i
  if ((UPDATE)); then
    [[ -f $INSTALLED ]] && COMP="$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("components",""))' "$INSTALLED" 2>/dev/null)"
    if [[ -n $COMP ]]; then
      known="$(cat "$KNOWN_F" 2>/dev/null || echo "core apps look login nvidia fork")"
      for i in "${!C_ID[@]}"; do
        if [[ " $COMP " == *" ${C_ID[$i]} "* ]]; then C_ON[i]=1
        elif [[ " $known " == *" ${C_ID[$i]} "* ]]; then C_ON[i]=0; fi
      done
    elif [[ ! -x $HOME/.local/lib/yami-niri/current/niri ]]; then
      # no record of the choice (a home-only install of an old version): the fork, a 5–25 minute
      # build and ~4 GB, only for those who already run it
      for i in "${!C_ID[@]}"; do [[ ${C_ID[$i]} == fork ]] && C_ON[i]=0; done
    fi
  fi
  return 0
}
c_name() {
  case $1 in
    core) t "niri и сеанс" "niri and session" ;;
    apps) t "программы" "apps" ;;
    tools) t "инструменты" "tools" ;;
    games) t "игры" "games" ;;
    look) t "тема программ" "app theme" ;;
    login) t "экран входа" "login screen" ;;
    nvidia) t "фикс NVIDIA" "NVIDIA fixes" ;;
    fork) t "yami-niri (форк)" "yami-niri (fork)" ;;
  esac
}
c_desc() {
  case $1 in
    core) t "niri, порталы, polkit, pipewire, сеть, bluetooth, буфер, скриншоты" "niri, portals, polkit, pipewire, network, bluetooth, clipboard, screenshots" ;;
    apps) t "kitty, fastfetch, pavucontrol, btop, udisks2, миниатюры видео, bsdtar, libmpv для плеера, gvfs: телефоны и сеть" "kitty, fastfetch, pavucontrol, btop, udisks2, video thumbnails, bsdtar, libmpv for the player, gvfs: phones and network" ;;
    tools) t "ночной свет (wlsunset), текст и QR со скриншота (tesseract, zbar)" "night light (wlsunset), text and QR from screenshots (tesseract, zbar)" ;;
    games) t "GameMode и MangoHud в палитре" "GameMode and MangoHud in the palette" ;;
    login) if [[ -n $OTHER_DM ]]; then t "свой, как экран блокировки: greetd + yami-greet, без Qt; заменит $OTHER_DM" "our own, like the lock screen: greetd + yami-greet, no Qt; replaces $OTHER_DM"
      else t "свой, как экран блокировки: greetd + yami-greet, без Qt, вход в niri" "our own, like the lock screen: greetd + yami-greet, no Qt, logs into niri"; fi ;;
    look) t "qt6ct + adw-gtk3 + Papirus: Qt, KDE и GTK в палитре райса" "qt6ct + adw-gtk3 + Papirus: Qt, KDE and GTK in the rice palette" ;;
    nvidia) t "переменные драйвера, профиль против утечки VRAM, драйвер с начала загрузки (пароль диска виден и на DisplayPort), NVENC в OBS на старом драйвере" "driver env, a VRAM leak profile, the driver from the start of boot (disk password visible on DisplayPort too), NVENC in OBS on an old driver" ;;
    fork)
      if ((LIGHT)); then t "наш niri из исходников: здесь ($MEM_GB ГБ, $THREADS потоков) сборка идёт около часа, поэтому выключен; пакетный niri — тот же стол" "our niri from source: here ($MEM_GB GB, $THREADS threads) the build takes about an hour, so it is off; the packaged niri is the same desktop"
      else t "наш niri из исходников, 5–25 мин: под этот процессор, tearing и realtime для игр; пакетный niri — запасной" "our niri from source, 5–25 min: built for this CPU, tearing and realtime for games; packaged niri stays as a fallback"; fi ;;
  esac
}
needs_root() { [[ $1 != fork ]]; }
can() { ! needs_root "$1" || ((SUDO)); }
on() { local i; for i in "${!C_ID[@]}"; do [[ ${C_ID[$i]} == "$1" ]] && return $((1 - C_ON[i])); done; return 1; }
off() { local i; for i in "${!C_ID[@]}"; do [[ ${C_ID[$i]} == "$1" ]] && C_ON[i]=0; done; }
wants() { local i; for i in "${!C_ID[@]}"; do [[ ${C_ID[$i]} == "$1" ]] && return $((1 - C_WANT[i])); done; return 1; }
# what was chosen is kept (C_WANT: an update without sudo still knows which packages the chosen
# components want, do_need_pkgs); what this run cannot do without root is switched off
components_limit() {
  local i
  C_WANT=("${C_ON[@]}")
  for i in "${!C_ID[@]}"; do can "${C_ID[$i]}" || C_ON[i]=0; done
  # yami-niri needs clang and the compositor's libraries: without root they must already be there
  ((SUDO)) || command -v clang >/dev/null || off fork
  return 0
}
