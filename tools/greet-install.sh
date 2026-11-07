#!/usr/bin/env bash
# Install (or update) the yami login screen: greetd starts yami-greet inside niri, from the next
# boot. Run from the repo as yourself; it builds yami-greet and asks for sudo once.
#   tools/greet-install.sh [--no-enable]   --no-enable: install, leave the login manager alone
# The installer runs the root half itself (as root):
#   greet-install.sh --system USER [--bin FILE] [--cursor DIR] [--no-enable] [--bin-only]
#   greet-install.sh --uninit        greetd off, the login manager it replaced on (OpenRC, runit)
# Init systems: systemd (display-manager.service), OpenRC (Alpine, Gentoo), runit (Void). Without
# logind/elogind the greeter user joins seatd's group, so niri gets a seat.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOOK=/var/lib/yami-greet/look STATE=/var/lib/yami-greet/state LOGD=/var/lib/yami-greet/log
REPLACED=/var/lib/yami-greet/replaced
CONF=/etc/greetd/config.toml
DMS=(sddm gdm lightdm lxdm ly emptty cosmic-greeter plasmalogin display-manager xdm slim)

init_kind() {
  if [[ -d /run/systemd/system ]]; then echo systemd
  elif command -v rc-update >/dev/null 2>&1 && [[ -d /etc/runlevels ]]; then echo openrc
  elif [[ -d /etc/sv ]] && runit_dir >/dev/null; then echo runit
  else echo other; fi
}
# where runit's enabled services are linked (Void: /var/service)
runit_dir() {
  local d
  for d in /var/service /etc/runit/runsvdir/default /run/runit/service; do
    [[ -d $d ]] && { echo "$d"; return 0; }
  done
  return 1
}

# the greetd user: the one its own config names, else the one the distro created
greeter_user() {
  local u
  u="$(awk -F'"' '/^[[:space:]]*\[/ { s = $0 } s ~ /default_session/ && /^[[:space:]]*user[[:space:]]*=/ { print $2; exit }' "$CONF" 2>/dev/null)"
  if [[ -n $u ]] && getent passwd "$u" >/dev/null; then echo "$u"; return; fi
  for u in greeter _greetd greetd _greeter; do getent passwd "$u" >/dev/null && { echo "$u"; return; }; done
  return 1
}

# niri needs a seat: logind (systemd) and elogind give one to every session; with seatd alone the
# greeter user has to be in seatd's group (Void: _seatd, Alpine and Gentoo: seat)
seat_setup() {
  local g="$1" grp
  [[ $INIT == systemd || -d /run/systemd/seats ]] && return 0
  for grp in _seatd seat; do
    if getent group "$grp" >/dev/null; then
      id -nG "$g" | tr ' ' '\n' | grep -qx "$grp" || usermod -aG "$grp" "$g"
      case $INIT in
        openrc) rc-update add seatd default >/dev/null 2>&1 || true ;;
        runit) [[ -d /etc/sv/seatd && ! -e $(runit_dir)/seatd ]] && ln -s /etc/sv/seatd "$(runit_dir)/" || true ;;
      esac
      echo "seat: $g joins $grp (seatd)"
      return 0
    fi
  done
  echo "no logind, elogind or seatd: niri of the login screen has no seat (the text login takes over)" >&2
}

# runit (Void): runsvdir starts or stops a service as soon as its link comes or goes, so a login
# manager switched now would end the running session or jump to its own VT. The switch waits for
# the next boot: a stage-1 script (core-services) does it once and removes itself
RUNIT_HOOK=/etc/runit/core-services/90-yami-greetd.sh
runit_switch() {
  if [[ -d /etc/runit/core-services ]]; then
    { echo "# yami: switches the login manager at this boot (tools/greet-install.sh), then removes itself"
      printf '%s\n' "$@"
      echo "rm -f $RUNIT_HOOK"; } >"$RUNIT_HOOK.new" && mv -f "$RUNIT_HOOK.new" "$RUNIT_HOOK"
    echo "runit: the login manager switches at the next boot (this session is untouched)"
  else
    local c
    for c; do sh -c "$c"; done
    echo "runit: switched now"
  fi
}
runit_svdir() { if [[ -d /etc/runit/runsvdir/default ]]; then echo /etc/runit/runsvdir/default; else runit_dir; fi; }

# greetd becomes the login manager; the ones it replaces are written down for --uninit
enable_greetd() {
  local s d lines=()
  case $INIT in
    systemd)
      systemctl enable -f greetd.service
      echo "greetd: the login manager from the next boot (this session is untouched)" ;;
    openrc)
      [[ -x /etc/init.d/greetd ]] || { echo "no /etc/init.d/greetd: install greetd's OpenRC script (Alpine: greetd-openrc)" >&2; return 1; }
      # rc-update only changes the runlevel: nothing starts or stops now
      for s in "${DMS[@]}"; do
        if [[ -e /etc/runlevels/default/$s ]]; then
          rc-update del "$s" default >/dev/null && note_replaced openrc "$s"
        fi
      done
      rc-update add greetd default >/dev/null
      echo "greetd: in the default runlevel from the next boot (this session is untouched)" ;;
    runit)
      [[ -d /etc/sv/greetd ]] || { echo "no /etc/sv/greetd: install greetd" >&2; return 1; }
      d="$(runit_svdir)"
      for s in "${DMS[@]}"; do
        [[ -L $d/$s ]] && { lines+=("rm -f '$d/$s'"); note_replaced runit "$s"; }
      done
      lines+=("[ -e '$d/greetd' ] || ln -s /etc/sv/greetd '$d/greetd'")
      runit_switch "${lines[@]}" ;;
    *) echo "unknown init system: enable the greetd service yourself" >&2 ;;
  esac
}
note_replaced() { grep -qsx "$1 $2" "$REPLACED" || echo "$1 $2" >>"$REPLACED"; }

# --uninit: greetd off, the managers enable_greetd turned off back on (systemd: install.sh does it)
uninit() {
  local kind s d lines=()
  INIT="$(init_kind)"
  case $INIT in
    openrc)
      rc-update del greetd default >/dev/null 2>&1 || true
      if [[ -r $REPLACED ]]; then
        while read -r kind s; do
          [[ $kind == openrc && $s =~ ^[A-Za-z0-9_.@-]+$ ]] && rc-update add "$s" default >/dev/null 2>&1 && echo "openrc: $s back in the default runlevel"
        done <"$REPLACED"
      fi ;;
    runit)
      d="$(runit_svdir)"
      lines=("rm -f '$d/greetd'")
      if [[ -r $REPLACED ]]; then
        while read -r kind s; do
          [[ $kind == runit && $s =~ ^[A-Za-z0-9_.@-]+$ && -d /etc/sv/$s ]] && lines+=("[ -e '$d/$s' ] || ln -s '/etc/sv/$s' '$d/$s'")
        done <"$REPLACED"
      fi
      runit_switch "${lines[@]}" ;;
  esac
  return 0
}

# SELinux (Fedora): files copied by root keep the label of where they came from, greetd could not run them
relabel() { command -v restorecon >/dev/null 2>&1 && restorecon -R "$@" 2>/dev/null || true; }

# the root half: the binary, folders, greet-session, fonts, cursor, greetd's config, the init system
system() {
  local user="" bin="" cursor="" enable=1 only=0
  while (($#)); do
    case "$1" in
      --system) user="$2"; shift ;;
      --bin) bin="$2"; shift ;;
      --cursor) cursor="$2"; shift ;;
      --no-enable) enable=0 ;;
      --bin-only) only=1 ;;
      *) echo "unknown: $1" >&2; exit 2 ;;
    esac
    shift
  done
  ((EUID == 0)) || { echo "greet-install.sh --system runs as root" >&2; exit 1; }
  [[ $user =~ ^[a-z_][a-z0-9_.-]*\$?$ ]] && getent passwd "$user" >/dev/null || { echo "no such user: $user" >&2; exit 1; }
  if [[ -n $bin ]]; then
    install -Dm755 "$bin" /usr/local/bin/.yami-greet.new && mv -f /usr/local/bin/.yami-greet.new /usr/local/bin/yami-greet
    relabel /usr/local/bin/yami-greet
  fi
  ((only)) && return 0
  command -v greetd >/dev/null || { echo "greetd is not installed" >&2; exit 1; }
  INIT="$(init_kind)"
  local g gg
  g="$(greeter_user)" || { echo "greetd has no greeter user" >&2; exit 1; }
  gg="$(id -gn "$g")"
  # what yami-greet reads: the look (yours, the shell writes it), its memory and log (the greeter's own)
  install -d -m755 /var/lib/yami-greet
  install -d -m755 -o "$user" -g "$(id -gn "$user")" "$LOOK"
  install -d -m700 -o "$g" -g "$gg" "$STATE"
  install -d -m755 -o "$g" -g "$gg" "$LOGD"
  # the first boot: this user and the niri session already picked. The folder is the greeter's: the
  # file is made in root's folder and renamed in, so root never writes through a link put there
  if [[ ! -e $STATE/last && ! -L $STATE/last ]]; then
    local tmp
    tmp="$(mktemp /var/lib/yami-greet/.last.XXXXXX)"
    printf '%s\nniri\n' "$user" >"$tmp" && chown "$g:$gg" "$tmp" && chmod 600 "$tmp" && mv -fT "$tmp" "$STATE/last" || rm -f -- "$tmp"
  fi
  install -Dm755 "$ROOT/system/greet/greet-session" /usr/local/lib/yami/greet-session
  install -Dm644 -t /usr/local/share/yami-greet/fonts "$ROOT/fonts/Tiny5.ttf" "$ROOT/fonts/DotGothic16-Regular.ttf" "$ROOT/fonts/OFL-tiny5.txt" "$ROOT/fonts/OFL-dotgothic16.txt"
  # the pixel cursor: niri of the login screen reads /usr/share/icons only
  if [[ -n $cursor && -d $cursor ]]; then
    rm -rf -- /usr/share/icons/yami-cursor
    cp -r "$cursor" /usr/share/icons/yami-cursor
    chmod -R a+rX /usr/share/icons/yami-cursor
  fi
  seat_setup "$g"
  # greetd's own config is kept once (./install.sh --uninstall puts it back); its terminal stays
  mkdir -p /etc/greetd
  if [[ -f $CONF ]] && ! head -n1 "$CONF" | grep -q '^# yami:' && [[ ! -e $CONF.yami-orig ]]; then
    cp -p "$CONF" "$CONF.yami-orig"
  fi
  local vt
  vt="$(awk '/^[[:space:]]*\[/ { s = $0 } s ~ /terminal/ && /^[[:space:]]*vt[[:space:]]*=/ { sub(/^[^=]*=[[:space:]]*/, ""); print; exit }' "$CONF.yami-orig" 2>/dev/null)"
  [[ $vt =~ ^([0-9]+|\"[a-z]+\")$ ]] || vt=1
  # without systemd nothing stops the getty on tty1 for greetd: the login screen takes VT 7
  [[ $INIT != systemd && $vt == 1 ]] && vt=7
  printf '%s\n' "# yami: the login screen (tools/greet-install.sh; ./install.sh --uninstall puts the old file back)" \
    "[terminal]" "vt = $vt" "" "[default_session]" "command = \"/usr/local/lib/yami/greet-session\"" "user = \"$g\"" >"$CONF.new"
  mv -f "$CONF.new" "$CONF"
  # the SDDM theme yami had before its own login screen
  rm -rf -- /usr/share/sddm/themes/yami /etc/sddm.conf.d/zz-yami.conf /etc/sddm.conf.d/10-yami.conf
  relabel /var/lib/yami-greet /usr/local/lib/yami /usr/local/share/yami-greet /usr/share/icons/yami-cursor /etc/greetd
  if [[ $INIT == systemd && -f /usr/lib/systemd/system/plymouth-quit.service ]]; then
    # plymouth: never let a hung boot splash block multi-user.target (see system/plymouth)
    for u in plymouth-quit plymouth-quit-wait; do
      install -Dm644 "$ROOT/system/plymouth/10-yami.conf" "/etc/systemd/system/$u.service.d/10-yami.conf"
    done
    systemctl daemon-reload
  fi
  ((enable)) && enable_greetd
  return 0
}

if [[ ${1:-} == --system ]]; then
  system "$@"
  exit 0
fi
if [[ ${1:-} == --uninit ]]; then
  ((EUID == 0)) || { echo "greet-install.sh --uninit runs as root" >&2; exit 1; }
  uninit
  exit 0
fi

ENABLE=()
for a; do case "$a" in --no-enable) ENABLE=(--no-enable) ;; *) echo "unknown: $a" >&2; exit 2 ;; esac; done
((EUID != 0)) || { echo "run it as yourself: it asks for the password itself" >&2; exit 1; }
SU=sudo
command -v sudo >/dev/null || SU=doas
if ! command -v greetd >/dev/null; then
  if command -v pacman >/dev/null; then $SU pacman -S --needed --noconfirm greetd greetd-agreety
  elif command -v dnf >/dev/null; then $SU dnf install -y greetd
  elif command -v apt-get >/dev/null; then $SU apt-get install -y greetd
  elif command -v zypper >/dev/null; then $SU zypper --non-interactive install greetd
  elif command -v xbps-install >/dev/null; then $SU xbps-install -y greetd
  elif command -v apk >/dev/null; then $SU apk add greetd greetd-agreety
  else echo "install greetd with your package manager first (./install.sh --login does it)" >&2; exit 1; fi
fi
# the updater's clone builds into the cache (install.sh did it a moment ago): the same target, so
# nothing is built twice; a checkout of your own builds into its shell/target
TARGET="${CARGO_TARGET_DIR:-}"
[[ -z $TARGET && $ROOT == "${XDG_DATA_HOME:-$HOME/.local/share}/yami/src" ]] && TARGET="${XDG_CACHE_HOME:-$HOME/.cache}/yami/target"
TARGET="${TARGET:-$ROOT/shell/target}"
cargo build --release --locked -p yami-greet --manifest-path "$ROOT/shell/Cargo.toml" --target-dir "$TARGET"
TMP="$(mktemp -d)"
trap 'rm -rf -- "$TMP"' EXIT
python3 -I "$ROOT/tools/theme.py" --cursor-to "$TMP/yami-cursor"
# the login manager greetd replaces (systemd): ./install.sh --uninstall enables it again; on OpenRC
# and runit the root half writes them down itself (/var/lib/yami-greet/replaced)
STATE_D="${XDG_STATE_HOME:-$HOME/.local/state}/yami"
prev="$(basename "$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)" .service)"
if [[ -n $prev && $prev != greetd && ! -s $STATE_D/login-dm ]]; then
  mkdir -p "$STATE_D" && printf '%s\n' "$prev" >"$STATE_D/login-dm"
fi
$SU "$ROOT/tools/greet-install.sh" --system "$(id -un)" --bin "$TARGET/release/yami-greet" --cursor "$TMP/yami-cursor" "${ENABLE[@]}"
# the look of the shell, now (it keeps it current itself)
yami-shell login sync >/dev/null 2>&1 || "$ROOT/shell/target/release/yami-shell" login sync >/dev/null 2>&1 || true
echo "login screen: yami-greet in greetd ($(/usr/local/bin/yami-greet --version))"
# SDDM (the login screen before 2.6, often installed by this installer) is off now, not removed: it
# may be KDE's. Once the new login screen has let you in, `yami doctor` prints the command to remove it
command -v sddm >/dev/null && echo "SDDM is turned off, not removed; after the first login here, yami doctor tells how to remove it if nothing else uses it"
