#!/usr/bin/env bash
# YAMI installer: the yami desktop on niri (the shell, files, viewer, player, login screen, themes
# and fonts) on any mainstream Linux distro. One command, from anywhere, as yourself (not root):
#
#   bash <(curl -fsSL https://raw.githubusercontent.com/Trendorin/yami/main/install.sh)
#
# It asks the language, shows what it will install and goes on Enter («c» changes the choice). Every
# default is picked for this machine: nothing below is needed for a normal install.
#
#   ./install.sh --yes            no questions, the defaults
#   ./install.sh --dry-run        show the plan, change nothing
#   ./install.sh --uninstall      remove the shell and its files, restore what it replaced
#   ./install.sh --update         rebuild from this tree, keep every setting, no sudo (what
#                                 `yami update` runs after checking the release)
#
# Choices (the interactive installer asks them; settings change them later):
#   --lang en|ru|pt-BR     --palette blood|ash|sakura|amber|venom|ice|dusk     --theme dark|light
#   --look matte|glass|light|solid     --font sans|pixel     --fetch mini|full
#
# Components: each on its own, --no-ID leaves it out, --ID takes it (defaults fit this machine;
# an update repeats the choice):
#   core    niri, portals, polkit, pipewire, network: what the shell needs
#   files   yami-files: folders, the file dialog of every app, «show in folder»
#   view    yami-view: pictures          play    yami-play: video and music (libmpv)
#   login   our login screen (off when another login manager is set)
#   apps    kitty, fastfetch, pavucontrol, btop
#   drives  udisks2, gvfs: drives mount by themselves, phones, network shares
#   tools   night light, screen text and QR (wlsunset, tesseract, zbar)
#   games   GameMode and MangoHud (on when Steam is here)
#   nvidia  niri's profile against VRAM growth (the proprietary driver): one file in /etc/nvidia,
#           nothing else of the driver (no environment, modprobe options or initramfs)
#   fork    yami-niri, built from source (5–25 min; off with less than 6 GB of memory or 4 threads)
#   --shell-only     niri and the shell alone: none of the above, nothing of others restyled
#   --restyle LIST   what yami may restyle in other programs (all, none, or a list of: gtk qt cursor
#                    kitty fastfetch btop firefox mangohud foot cava starship); --no-restyle = none.
#                    Kept in the shell's settings (themeApps), settings → color → apps changes it
#   --no-sudo        the home folder only: no packages, no system files
#
# Rarely needed: --no-monocraft (skip the 31 MB terminal font), --no-count (do not count this
# install, docs/SECURITY.md), --keep-build (keep the ~4 GB yami-niri sources), --no-anim,
# --print-packages (the package lists for this distro, CI checks them).
#
# Families: Arch, Fedora (incl. atomic), Debian/Ubuntu, openSUSE, Void, Alpine, Gentoo. NixOS: the
# flake (flake.nix, docs/PACKAGING.md); this script installs only the home folder there. Anything
# replaced is kept next to itself as *.bak-DATE. Log: ~/.cache/yami/install.log. Each module in
# installer/*.sh holds one job and adds its own steps (plan_*); this file is the flags and their order.
set -uo pipefail
# the whole script is one block and the modules are read whole when sourced: bash parses all of it
# before running, so an update of these files mid-run (git pull, an editor) cannot shift lines
{

# ── bootstrap: started outside the repo (curl | bash) — fetch it and run from there ──
YAMI_REPO="${YAMI_REPO:-https://github.com/Trendorin/yami.git}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd)"
if [[ ! -f $ROOT/shell/Cargo.toml || ! -f $ROOT/installer/ui.sh ]]; then
  SRC_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/yami/src"
  command -v git >/dev/null || { echo "yami: git is needed to fetch the installer (install git first)" >&2; exit 1; }
  if [[ -d $SRC_DIR/.git ]]; then
    # a clone of an older address follows the current one
    [[ "$(git -C "$SRC_DIR" remote get-url origin 2>/dev/null)" == "$YAMI_REPO" ]] || git -C "$SRC_DIR" remote set-url origin "$YAMI_REPO"
    git -C "$SRC_DIR" pull --ff-only --quiet || echo "yami: could not update $SRC_DIR, using it as is" >&2
  else
    mkdir -p "${SRC_DIR%/*}"
    # history without file contents: small like --depth 1, but updates can compare versions
    git clone --filter=blob:none "$YAMI_REPO" "$SRC_DIR" || { echo "yami: git clone $YAMI_REPO failed" >&2; exit 1; }
  fi
  # curl … | bash: give the interactive installer the terminal back
  if [[ ! -t 0 ]] && { : </dev/tty; } 2>/dev/null; then exec bash "$SRC_DIR/install.sh" "$@" </dev/tty; fi
  exec bash "$SRC_DIR/install.sh" "$@"
fi

# ── where things go ─────────────────────────────────────────────────────────
CFG="${XDG_CONFIG_HOME:-$HOME/.config}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/yami"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}/yami"
BIN="$HOME/.local/bin"
STAMP="$(date +%Y%m%d-%H%M%S)"
# settings of this shell; an install under the old name (DARK-4RMY) is read until it is moved
SET_JSON="$CFG/yami-shell.json"
[[ ! -f $SET_JSON && -f $CFG/dark4rmy-shell.json ]] && SET_JSON="$CFG/dark4rmy-shell.json"
LOG="$CACHE/install.log"
MISSING_F="$CACHE/missing-packages"
RUST_MIN="1.89"
NIRI_TAG="v26.04"
# ext-background-effect (blur only under the shell panels, not their shadow margins) and the
# config syntax need niri 26.04 or newer
NIRI_MIN="26.04"
# pinned and checked: a changed upload is refused (kitty then uses Terminess from the repo)
MONOCRAFT_URL="https://github.com/IdreesInc/Monocraft/releases/download/v4.2.1/Monocraft-nerd-fonts-patched.ttc"
MONOCRAFT_SHA256="67f88ff9e7c6560f6cf60fb062fd353a72f62dc2654462950c65b63ed53d9754"
[[ -d $HOME/.cargo/bin ]] && export PATH="$HOME/.cargo/bin:$PATH"

# ── flags ───────────────────────────────────────────────────────────────────
YES=0 SUDO=1 SHELL_ONLY=0 RESTYLE="" COMP_FLAGS=() PALETTE="" LOOK_OPT="" THEME_OPT="" LANG_OPT="" FONT_OPT="" FETCH_OPT="" ANIM=1 DRY=0 UNINSTALL=0 UPDATE=0 COUNT=1 KEEP_BUILD=0 MONO="" PRINT_PKGS=0
while (($#)); do
  case "$1" in
    -y | --yes) YES=1 ;;
    --no-sudo) SUDO=0 ;;
    --shell-only) SHELL_ONLY=1 ;;
    --restyle) RESTYLE="${2:-}"; shift ;;
    --no-restyle) RESTYLE=none ;;
    --font) FONT_OPT="${2:-}"; shift ;;
    --fetch) FETCH_OPT="${2:-}"; shift ;;
    --palette) PALETTE="${2:-}"; shift ;;
    --look) LOOK_OPT="${2:-}"; shift ;;
    --theme) THEME_OPT="${2:-}"; shift ;;
    --lang) LANG_OPT="${2:-}"; shift ;;
    --no-anim) ANIM=0 ;;
    --dry-run) DRY=1 ;;
    --print-packages) PRINT_PKGS=1 ;;
    --no-count) COUNT=0 ;;
    --keep-build) KEEP_BUILD=1 ;;
    --no-monocraft) MONO=0 ;;
    --monocraft) MONO=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --update) UPDATE=1 YES=1 ANIM=0 SUDO=0 ;;
    -h | --help) sed -n '2,/^set -uo/p' "${BASH_SOURCE[0]:-$0}" | sed -e '$d' -e 's/^# \{0,1\}//'; exit 0 ;;
    # a component: --no-files, --games… (components.sh knows the names and refuses others)
    --no-[a-z]*) COMP_FLAGS+=("${1#--no-}=0") ;;
    --[a-z]*) COMP_FLAGS+=("${1#--}=1") ;;
    *) echo "unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done
[[ -t 0 && -t 1 ]] || { YES=1; ANIM=0; }
# git never asks for a login on the terminal: a moved or private repo fails instead of hanging
export GIT_TERMINAL_PROMPT=0

# ── the modules: each only defines (installer/*.sh) ─────────────────────────
for m in ui detect root packages components restyle menu build apps desktop niri login nvidia migrate uninstall summary; do
  # shellcheck source=/dev/null
  . "$ROOT/installer/$m.sh" || { echo "yami: $ROOT/installer/$m.sh is missing" >&2; exit 1; }
done
unset m

detect_distro
if ((PRINT_PKGS)); then
  packages_for_family
  print_packages
  exit 0
fi
# `sudo ./install.sh` would put the desktop into root's home: the installer asks for the password
# itself, only where it is needed (containers and CI run as root on purpose: YAMI_AS_ROOT=1)
if ((EUID == 0)) && [[ -z ${YAMI_AS_ROOT:-} && -z ${CI:-} ]]; then
  echo "yami: run the installer as yourself, not as root: it asks for the password when it needs it" >&2
  echo "      ./install.sh   (a root shell on purpose, e.g. a container: YAMI_AS_ROOT=1)" >&2
  exit 1
fi
mkdir -p "$CACHE"
: >"$LOG"
: >"$MISSING_F"

# ── language: the flag, then what the shell is set to (update), then the system ──
L=""
if [[ -n $LANG_OPT ]]; then
  L="$(lang_code "$LANG_OPT")"
  [[ -n $L ]] || { echo "--lang: en, ru or pt-BR" >&2; exit 2; }
elif ((UPDATE)) && [[ -f $SET_JSON ]]; then
  L="$(lang_code "$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("lang",""))' "$SET_JSON" 2>/dev/null)")"
fi
if [[ -z $L ]]; then
  L="$(lang_code "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}")"
  L="${L:-en}"
  LANG_ASK=1
else
  LANG_ASK=0
fi
# ${#s} must count characters, not bytes
case "$(locale charmap 2>/dev/null)" in UTF-8) ;; *) export LC_ALL=C.UTF-8 ;; esac

detect_machine
detect_login
packages_for_family
choices_init
paint
ui_traps
components_init
restyle_init
components_limit

# ── go: each module adds its own steps (plan_*), run_steps runs them in this order ──
((UNINSTALL)) && uninstall

if ((UPDATE)); then
  # rebuild from this tree with the choices of the first install; no packages, no sudo
  say "$(t "обновление из" "update from" "atualizar de") $ROOT"
  plan_build    # build.sh: the shell and the chosen apps
  plan_apps     # apps.sh: yami-files, yami-view, yami-play (or out, when unticked)
  plan_desktop  # desktop.sh: fonts, niri config, everything from the palette
  plan_legacy   # migrate.sh
  plan_record   # desktop.sh: install record, packages-needed, counter, restart
  run_steps
  fork_after    # niri.sh: yami-niri when its patch series moved
  login_after   # login.sh: offers the new login screen
  summary_update
fi

if ((!YES)); then
  printf '\e[H\e[J'
  banner 1
  ask_all
fi
paint
[[ -t 1 ]] && printf '\e[H\e[J'
banner $((YES))
root_begin      # root.sh: the password once

# root steps first: the root shell closes before the build (run_steps, root.sh)
plan_packages   # packages.sh
plan_niri       # niri.sh: upstream niri when the packaged one is too old
plan_nvidia     # nvidia.sh: niri's VRAM profile
plan_services   # desktop.sh
plan_migrate    # migrate.sh: DARK-4RMY
plan_login      # login.sh: greetd (the last root step)
plan_build
plan_apps
plan_desktop
plan_legacy
plan_record
run_steps
root_end        # root.sh
login_after     # login.sh: yami-greet itself
fork_after      # niri.sh: yami-niri (minutes)
summary_install # summary.sh
exit 0
}
