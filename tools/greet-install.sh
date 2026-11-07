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
if ! command -v greetd >/dev/null; then
  if command -v pacman >/dev/null; then sudo pacman -S --needed --noconfirm greetd greetd-agreety
  elif command -v dnf >/dev/null; then sudo dnf install -y greetd
  elif command -v apt-get >/dev/null; then sudo apt-get install -y greetd
  elif command -v zypper >/dev/null; then sudo zypper --non-interactive install greetd
  elif command -v xbps-install >/dev/null; then sudo xbps-install -y greetd
  elif command -v apk >/dev/null; then sudo apk add greetd greetd-agreety
  else echo "install greetd with your package manager first (./install.sh --login does it)" >&2; exit 1; fi
fi
cargo build --release --locked -p yami-greet --manifest-path "$ROOT/shell/Cargo.toml"
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
sudo "$ROOT/tools/greet-install.sh" --system "$(id -un)" --bin "$ROOT/shell/target/release/yami-greet" --cursor "$TMP/yami-cursor" "${ENABLE[@]}"
# the look of the shell, now (it keeps it current itself)
yami-shell login sync >/dev/null 2>&1 || "$ROOT/shell/target/release/yami-shell" login sync >/dev/null 2>&1 || true
echo "login screen: yami-greet in greetd ($(/usr/local/bin/yami-greet --version))"
