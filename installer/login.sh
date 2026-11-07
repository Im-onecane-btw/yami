# The login screen, greetd + yami-greet. greetd (a small daemon in Rust, every
# distro has it) does PAM and starts the session; yami-greet draws the login screen inside niri with
# the lock screen's look. The root half is tools/greet-install.sh --system. An update (no sudo) only
# offers it: now from a terminal, else one button in settings → updates. --uninstall takes it out and
# brings back the login manager it replaced.
# shellcheck shell=bash

# the installed login screen is older than this build (or still the SDDM theme of before 2.6)
greet_stale() {
  [[ -d /usr/share/sddm/themes/yami ]] && return 0
  [[ -x /usr/local/bin/yami-greet ]] || return 1
  ! cmp -s "$CACHE/target/release/yami-greet" /usr/local/bin/yami-greet || ! cmp -s "$ROOT/system/greet/greet-session" /usr/local/lib/yami/greet-session
}

# the root half before the build (greetd, folders, greet-session, fonts, cursor); yami-greet itself is
# installed after it (greet_bin), greet-session meanwhile falls back to greetd's text greeter, so a
# failed build never leaves the machine without a login
do_greet() {
  command -v greetd >/dev/null || return 1
  local acc=(--accent "#${PAL_HEX[$pal]}")
  ((theme == 1)) && acc+=(--light)
  # the pixel cursor on the login screen too: niri there reads /usr/share/icons only
  rm -rf -- "$CACHE/yami-cursor"
  python3 -I "$ROOT/tools/theme.py" --cursor-to "$CACHE/yami-cursor" "${acc[@]}" || return 1
  if ((SYSTEMD)) && [[ -n $OTHER_DM && ! -s $STATE/login-dm ]]; then
    # the login manager it replaces, from the next boot (this session is untouched); --uninstall brings it back
    mkdir -p "$STATE" && printf '%s\n' "$OTHER_DM" >"$STATE/login-dm"
  fi
  $SU "$ROOT/tools/greet-install.sh" --system "$(id -un)" --cursor "$CACHE/yami-cursor" || return 1
  rm -rf -- "$CACHE/yami-cursor"
}
# after the build: the binary, root-owned in /usr/local/bin (the root shell is closed by then: the
# password is asked once more, like for the yami-niri capability), and the shell's look for it
greet_bin() {
  [[ -x $CACHE/target/release/yami-greet ]] || return 1
  $SU "$ROOT/tools/greet-install.sh" --system "$(id -un)" --bin "$CACHE/target/release/yami-greet" --bin-only || return 1
  "$BIN/yami-shell" login sync >/dev/null 2>&1 || true
}


# update: the login screen is root's (greetd, /usr/local/bin), an update has no sudo. From a terminal
# it asks now; from the shell (no terminal) settings → updates gets a button for the same command
# (packages-needed: «login PATH», need.rs). SDDM is turned off, never removed. `wants`, not `on`:
# without sudo every root component is off in C_ON
login_offer() {
  local msg a f="$STATE/packages-needed"
  wants login && ((!DRY && !BUILD_FAILED)) && greet_stale || return 0
  mkdir -p "$STATE" && printf 'login %s\n' "$ROOT/tools/greet-install.sh" >>"$f"
  if [[ -d /usr/share/sddm/themes/yami ]]; then
    msg="$(t "новый экран входа: свой, вместо SDDM (SDDM выключится, но останется — ./install.sh --uninstall вернёт его)" "the new login screen: our own instead of SDDM (SDDM is turned off, not removed; ./install.sh --uninstall brings it back)")"
  else
    msg="$(t "экран входа обновился" "the login screen changed")"
  fi
  if [[ -t 0 && -t 1 && -n $SU ]]; then
    printf '\n  %s%s%s\n  %s %s[%s]%s ' "$T" "$msg" "$R" "$(t "поставить сейчас?" "install it now?")" "$F" "$(t "enter — да, n — потом" "enter yes, n later")" "$R"
    read -r a || a=n
    if [[ ! $a =~ ^[nNнН] ]] && "$ROOT/tools/greet-install.sh"; then
      sed -i '/^login /d' "$f"
      grep -q '^pkgs ' "$f" || rm -f "$f"
    else
      note "$(t "потом: настройки → обновления, или $ROOT/tools/greet-install.sh" "later: settings → updates, or $ROOT/tools/greet-install.sh")"
    fi
  else
    note "$msg: $(t "настройки → обновления, одна кнопка (или $ROOT/tools/greet-install.sh, нужен sudo)" "settings → updates, one button (or $ROOT/tools/greet-install.sh, needs sudo)")"
  fi
}

# the login screen: yami-greet, its files, cursor and greetd config go (greetd gets its own config
# back), the login manager it replaced (do_greet wrote it down) comes back from the next boot;
# greetd itself stays. The plymouth and NVIDIA fixes stay too: they help without yami.
uninstall_login() {
  local dm="" f files=() now
  [[ -r $STATE/login-dm ]] && dm="$(head -n1 "$STATE/login-dm")"
  [[ $dm =~ ^[A-Za-z0-9_.@-]+$ ]] || dm=""
  # the manager comes back only while greetd is still the one: another chosen since then stays
  now="$(basename "$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)" .service)"
  if [[ -n $dm && $now != greetd ]]; then
    dm=""
    rm -f "$STATE/login-dm"
  fi
  for f in /usr/local/bin/yami-greet /usr/local/lib/yami/greet-session /usr/local/share/yami-greet /var/lib/yami-greet /usr/share/icons/yami-cursor \
    /usr/share/sddm/themes/yami /etc/sddm.conf.d/zz-yami.conf /etc/sddm.conf.d/10-yami.conf; do
    [[ -e $f ]] && files+=("$f")
  done
  local ours=0
  grep -qs '^# yami:' /etc/greetd/config.toml && ours=1
  ((${#files[@]} || ours)) || [[ -n $dm ]] || return 0
  local cmd="rm -rf ${files[*]}"
  # OpenRC, runit: greetd off and the managers it replaced back on (greet-install.sh wrote them down)
  ((SYSTEMD)) || cmd="$ROOT/tools/greet-install.sh --uninit; $cmd"
  ((ours)) && cmd+="; mv -f /etc/greetd/config.toml.yami-orig /etc/greetd/config.toml"
  [[ -n $dm ]] && cmd+="; systemctl enable -f $dm.service"
  if ((EUID != 0)) && { ((!SUDO)) || [[ -z $SU ]] || ! root_start; }; then
    warn "$(t "экран входа не убран, нужен root:" "the login screen is not removed, root is needed:") sudo sh -c '$cmd'"
    return 0
  fi
  if ((!SYSTEMD)) && [[ -e /var/lib/yami-greet ]]; then
    $SU "$ROOT/tools/greet-install.sh" --uninit || warn "$(t "greetd не выключен: включи прежний менеджер входа сам" "greetd is not turned off: enable your old login manager yourself")"
  fi
  ((${#files[@]})) && $SU rm -rf -- "${files[@]}"
  if ((ours)); then
    # greetd без нашего конфига: его собственный, если был, иначе текстовое приветствие
    if [[ -f /etc/greetd/config.toml.yami-orig ]]; then
      $SU mv -f /etc/greetd/config.toml.yami-orig /etc/greetd/config.toml
    else
      printf '%s\n' "[terminal]" "vt = 1" "" "[default_session]" 'command = "agreety --cmd /bin/sh"' "user = \"$(awk -F'"' '/^user/ { print $2; exit }' /etc/greetd/config.toml)\"" | $SU tee /etc/greetd/config.toml.new >/dev/null &&
        $SU mv -f /etc/greetd/config.toml.new /etc/greetd/config.toml
    fi
  fi
  say "$(t "экран входа yami убран" "the yami login screen is removed")"
  if [[ -n $dm ]] && ((SYSTEMD)); then
    if $SU systemctl enable -f "$dm.service"; then
      say "$(t "менеджер входа: снова $dm со следующей загрузки" "login manager: $dm again from the next boot")"
      rm -f "$STATE/login-dm"
    else
      warn "$(t "не включился $dm: sudo systemctl enable -f $dm.service" "$dm did not enable: sudo systemctl enable -f $dm.service")"
    fi
  elif ((SYSTEMD)) && [[ $now == greetd ]]; then
    # installed before the replaced manager was written down: name the ones on this system
    local u others=()
    for u in sddm gdm lightdm lxdm ly cosmic-greeter plasmalogin; do
      [[ -f /usr/lib/systemd/system/$u.service || -f /etc/systemd/system/$u.service ]] && others+=("$u")
    done
    ((${#others[@]})) && note "$(t "greetd остаётся менеджером входа (текстовый вход); вернуть прежний:" "greetd stays the login manager (text login); to switch back:") sudo systemctl enable -f ${others[0]}.service$( ((${#others[@]} > 1)) && printf ' (%s)' "${others[*]}")"
  fi
  root_stop
}
