#!/usr/bin/env bash
# Install (or update) the yami login screen: greetd starts yami-greet inside niri, from the next
# boot. Run from the repo as yourself; it builds yami-greet and asks for sudo once.
#   tools/greet-install.sh [--no-enable]   --no-enable: install, leave display-manager.service alone
# The installer runs the root half itself (as root):
#   greet-install.sh --system USER [--bin FILE] [--cursor DIR] [--no-enable] [--bin-only]
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOOK=/var/lib/yami-greet/look STATE=/var/lib/yami-greet/state
CONF=/etc/greetd/config.toml

# the greetd user: the one its own config names, else the one the distro created
greeter_user() {
  local u
  u="$(awk -F'"' '/^[[:space:]]*\[/ { s = $0 } s ~ /default_session/ && /^[[:space:]]*user[[:space:]]*=/ { print $2; exit }' "$CONF" 2>/dev/null)"
  if [[ -n $u ]] && getent passwd "$u" >/dev/null; then echo "$u"; return; fi
  for u in greeter _greetd greetd; do getent passwd "$u" >/dev/null && { echo "$u"; return; }; done
  return 1
}

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
  fi
  ((only)) && return 0
  command -v greetd >/dev/null || { echo "greetd is not installed" >&2; exit 1; }
  local g
  g="$(greeter_user)" || { echo "greetd has no greeter user" >&2; exit 1; }
  # what yami-greet reads: the look (yours, the shell writes it), its memory (the greeter's own)
  install -d -m755 /var/lib/yami-greet
  install -d -m755 -o "$user" -g "$(id -gn "$user")" "$LOOK"
  install -d -m700 -o "$g" -g "$(id -gn "$g")" "$STATE"
  # the first boot: this user and the niri session already picked
  if [[ ! -e $STATE/last ]]; then
    printf '%s\nniri\n' "$user" >"$STATE/last"
    chown "$g:$(id -gn "$g")" "$STATE/last"
  fi
  install -Dm755 "$ROOT/system/greet/greet-session" /usr/local/lib/yami/greet-session
  install -Dm644 -t /usr/local/share/yami-greet/fonts "$ROOT/fonts/Tiny5.ttf" "$ROOT/fonts/DotGothic16-Regular.ttf" "$ROOT/fonts/OFL-tiny5.txt" "$ROOT/fonts/OFL-dotgothic16.txt"
  # the pixel cursor: niri of the login screen reads /usr/share/icons only
  if [[ -n $cursor && -d $cursor ]]; then
    rm -rf -- /usr/share/icons/yami-cursor
    cp -r "$cursor" /usr/share/icons/yami-cursor
    chmod -R a+rX /usr/share/icons/yami-cursor
  fi
  # greetd's own config is kept once (./install.sh --uninstall puts it back); its terminal stays
  mkdir -p /etc/greetd
  if [[ -f $CONF ]] && ! head -n1 "$CONF" | grep -q '^# yami:' && [[ ! -e $CONF.yami-orig ]]; then
    cp -p "$CONF" "$CONF.yami-orig"
  fi
  local vt
  vt="$(awk '/^[[:space:]]*\[/ { s = $0 } s ~ /terminal/ && /^[[:space:]]*vt[[:space:]]*=/ { sub(/^[^=]*=[[:space:]]*/, ""); print; exit }' "$CONF.yami-orig" 2>/dev/null)"
  [[ $vt =~ ^([0-9]+|\"[a-z]+\")$ ]] || vt=1
  printf '%s\n' "# yami: the login screen (tools/greet-install.sh; ./install.sh --uninstall puts the old file back)" \
    "[terminal]" "vt = $vt" "" "[default_session]" "command = \"/usr/local/lib/yami/greet-session\"" "user = \"$g\"" >"$CONF.new"
  mv -f "$CONF.new" "$CONF"
  # the SDDM theme yami had before its own login screen
  rm -rf -- /usr/share/sddm/themes/yami /etc/sddm.conf.d/zz-yami.conf /etc/sddm.conf.d/10-yami.conf
  if [[ -d /run/systemd/system ]]; then
    # plymouth: never let a hung boot splash block multi-user.target (see system/plymouth)
    if [[ -f /usr/lib/systemd/system/plymouth-quit.service ]]; then
      for u in plymouth-quit plymouth-quit-wait; do
        install -Dm644 "$ROOT/system/plymouth/10-yami.conf" "/etc/systemd/system/$u.service.d/10-yami.conf"
      done
      systemctl daemon-reload
    fi
    if ((enable)); then
      systemctl enable -f greetd.service
      echo "greetd: the login manager from the next boot (this session is untouched)"
    fi
  elif ((enable)); then
    echo "no systemd: enable the greetd service with your init system" >&2
  fi
}

if [[ ${1:-} == --system ]]; then
  system "$@"
  exit 0
fi

ENABLE=()
for a; do case "$a" in --no-enable) ENABLE=(--no-enable) ;; *) echo "unknown: $a" >&2; exit 2 ;; esac; done
if ! command -v greetd >/dev/null; then
  if command -v pacman >/dev/null; then sudo pacman -S --needed --noconfirm greetd greetd-agreety
  else echo "install greetd with your package manager first" >&2; exit 1; fi
fi
cargo build --release --locked -p yami-greet --manifest-path "$ROOT/shell/Cargo.toml"
TMP="$(mktemp -d)"
trap 'rm -rf -- "$TMP"' EXIT
python3 -I "$ROOT/tools/theme.py" --cursor-to "$TMP/yami-cursor"
# the login manager greetd replaces: ./install.sh --uninstall enables it again
STATE_D="${XDG_STATE_HOME:-$HOME/.local/state}/yami"
prev="$(basename "$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)" .service)"
if [[ -n $prev && $prev != greetd && ! -s $STATE_D/login-dm ]]; then
  mkdir -p "$STATE_D" && printf '%s\n' "$prev" >"$STATE_D/login-dm"
fi
sudo "$ROOT/tools/greet-install.sh" --system "$(id -un)" --bin "$ROOT/shell/target/release/yami-greet" --cursor "$TMP/yami-cursor" "${ENABLE[@]}"
# the look of the shell, now (it keeps it current itself)
yami-shell login sync >/dev/null 2>&1 || "$ROOT/shell/target/release/yami-shell" login sync >/dev/null 2>&1 || true
echo "login screen: yami-greet in greetd ($(/usr/local/bin/yami-greet --version))"
