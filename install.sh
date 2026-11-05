#!/usr/bin/env bash
# YAMI installer: the yami desktop on niri (yami-niri, the shell, files, viewer, login screen,
# themes and fonts) on any mainstream Linux distro. One command, from anywhere:
#
#   bash <(curl -fsSL https://raw.githubusercontent.com/Trendorin/yami/main/install.sh)
#
# (run outside a checkout it clones the repo into ~/.local/share/yami/src and starts from there)
#
#   ./install.sh                  interactive: language, components, palette, fonts, summary
#   ./install.sh --yes            no questions, sensible defaults
#   ./install.sh --lang en|ru     interface language (installer, shell, key hints)
#   ./install.sh --palette ash    blood ash sakura amber venom ice dusk
#   ./install.sh --look light     panel transparency: matte glass light solid
#   ./install.sh --theme light    dark (default) or light: the shell and every app it recolours
#   ./install.sh --font sans      app font: sans (readable, default) or pixel (Tiny5 everywhere)
#   ./install.sh --fetch mini     fastfetch: mini (who, system, desktop; default) or full (+ hardware)
#   ./install.sh --no-fork        packaged niri instead of yami-niri (built from source, 5–25 min;
#                                 off by default with less than 6 GB of memory or 4 threads, --fork turns it on)
#   ./install.sh --no-tools       skip night light, screen text recognition and QR (wlsunset, tesseract, zbar)
#   ./install.sh --games          GameMode and MangoHud (on by default when Steam is here)
#   ./install.sh --no-games       leave them out
#   ./install.sh --login          SDDM with the yami theme even if another login manager is set
#   ./install.sh --no-login       leave the login manager alone
#   ./install.sh --no-sudo        home directory only: no packages, no system files
#   ./install.sh --no-anim        no animations
#   ./install.sh --dry-run        show the plan, change nothing
#   ./install.sh --print-packages the package lists for this distro, one per line (CI checks them)
#   ./install.sh --no-count       do not count this install (see docs/SECURITY.md, «Install count»)
#   ./install.sh --keep-build     keep the yami-niri sources after building it (~4 GB; removed by default)
#   ./install.sh --no-monocraft   skip the 31 MB Monocraft font: the terminal uses Terminess (--monocraft undoes it)
#   ./install.sh --uninstall      remove the shell and dotfiles, restore backups
#   ./install.sh --update         rebuild from this tree, keep every setting, no sudo (what
#                                 `yami update apply` runs after checking the release)
#
# Supported families: Arch, Fedora (incl. atomic), Debian/Ubuntu, openSUSE, Void,
# Alpine, Gentoo; NixOS gets the dotfiles and the shell, packages are listed for
# configuration.nix. Anything replaced is kept next to itself as *.bak-DATE.
# Log: ~/.cache/yami/install.log
set -uo pipefail
# the whole script is one block: bash parses it before running, so an update of this file
# mid-run (git pull, an editor) cannot make it execute shifted lines
{

# ── bootstrap: started outside the repo (curl | bash) — fetch it and run from there ──
YAMI_REPO="${YAMI_REPO:-https://github.com/Trendorin/yami.git}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd)"
if [[ ! -f $ROOT/shell/Cargo.toml || ! -f $ROOT/fork/yami-niri.sh ]]; then
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

CFG="${XDG_CONFIG_HOME:-$HOME/.config}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/yami"
STATE="$HOME/.local/state/yami"
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

YES=0 SUDO=1 FORK_FLAG="" LOGIN_FLAG="" GAMES_FLAG="" TOOLS_FLAG=1 PALETTE="" LOOK_OPT="" THEME_OPT="" LANG_OPT="" FONT_OPT="" FETCH_OPT="" ANIM=1 DRY=0 UNINSTALL=0 UPDATE=0 COUNT=1 KEEP_BUILD=0 MONO="" PRINT_PKGS=0
while (($#)); do
  case "$1" in
    -y | --yes) YES=1 ;;
    --no-sudo) SUDO=0 ;;
    --fork) FORK_FLAG=1 ;;
    --no-fork) FORK_FLAG=0 ;;
    --games) GAMES_FLAG=1 ;;
    --no-games) GAMES_FLAG=0 ;;
    --tools) TOOLS_FLAG=1 ;;
    --no-tools) TOOLS_FLAG=0 ;;
    --font) FONT_OPT="${2:-}"; shift ;;
    --fetch) FETCH_OPT="${2:-}"; shift ;;
    --login) LOGIN_FLAG=1 ;;
    --no-login) LOGIN_FLAG=0 ;;
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
    *) echo "unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done
[[ -t 0 && -t 1 ]] || { YES=1; ANIM=0; }
# git never asks for a login on the terminal: a moved or private repo fails instead of hanging
export GIT_TERMINAL_PROMPT=0
mkdir -p "$CACHE"
: >"$LOG"
: >"$MISSING_F"

# ── language ────────────────────────────────────────────────────────────────
L="${LANG_OPT:-}"
if [[ -z $L ]] && ((UPDATE)) && [[ -f $SET_JSON ]]; then
  L="$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("lang",""))' "$SET_JSON" 2>/dev/null)"
  [[ $L == ru || $L == en ]] || L=""
fi
if [[ -z $L ]]; then
  case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in ru*) L=ru ;; *) L=en ;; esac
  LANG_ASK=1
else
  LANG_ASK=0
fi
[[ $L == ru || $L == en ]] || { echo "--lang: ru or en" >&2; exit 2; }
# ${#s} must count characters, not bytes
case "$(locale charmap 2>/dev/null)" in UTF-8) ;; *) export LC_ALL=C.UTF-8 ;; esac
t() { if [[ $L == ru ]]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }

# ── distro ──────────────────────────────────────────────────────────────────
OS_NAME="Linux" FAM="" ATOMIC=0
if [[ -r /etc/os-release ]]; then
  # shellcheck disable=SC1091
  OS_NAME="$(. /etc/os-release; printf '%s' "${PRETTY_NAME:-${NAME:-Linux}}")"
  for id in $(. /etc/os-release; printf '%s %s' "${ID:-}" "${ID_LIKE:-}"); do
    case "$id" in
      nixos) FAM=nixos ;;
      arch | archlinux | archarm) FAM=arch ;;
      fedora | rhel | centos) FAM=fedora ;;
      debian | ubuntu) FAM=debian ;;
      suse | opensuse | opensuse-tumbleweed | opensuse-leap | sles) FAM=suse ;;
      void) FAM=void ;;
      alpine | postmarketos) FAM=alpine ;;
      gentoo | funtoo) FAM=gentoo ;;
    esac
    [[ -n $FAM ]] && break
  done
fi
if [[ -z $FAM ]]; then
  for pm in pacman:arch dnf:fedora apt-get:debian zypper:suse xbps-install:void apk:alpine emerge:gentoo; do
    command -v "${pm%%:*}" >/dev/null && { FAM=${pm#*:}; break; }
  done
fi
[[ $FAM == fedora && -e /run/ostree-booted ]] && ATOMIC=1
case "$FAM" in
  arch) PM=pacman ;; fedora) PM=$( ((ATOMIC)) && echo rpm-ostree || echo dnf) ;; debian) PM=apt ;;
  suse) PM=zypper ;; void) PM=xbps ;; alpine) PM=apk ;; gentoo) PM=emerge ;; nixos) PM=nix ;; *) PM="" ;;
esac

SU=""
if ((EUID == 0)); then SU=""; elif command -v sudo >/dev/null; then SU=sudo; elif command -v doas >/dev/null; then SU=doas; else SUDO=0; fi
[[ -z $PM || $PM == nix ]] && SUDO=0

# ── root: one password for the whole install ────────────────────────────────
# Every root command goes to one root shell started once (root_start): sudo or doas asks a single
# time, even doas without `persist` and even when a step falls back to installing packages one by
# one (it used to ask for each). The cached ticket is dropped right after, so nothing else run by
# the install (the crates' build scripts, minutes of them) can use it; the channel to the root shell
# is two pipes only this script holds: bash closes {var} descriptors in the programs it starts.
# A command runs with the caller's stdin, stdout and stderr (through /proc), so pipes into `tee`
# and the log work as before (a socket cannot be reopened: /dev/null instead); it returns the
# command's exit code.
ROOT_W="" ROOT_R=""
root_start() {
  local tool=$SU rc
  [[ -n $tool ]] || return 0
  coproc ROOTSH {
    # shellcheck disable=SC2016
    "$tool" bash -c 'while IFS= read -r p && IFS= read -r c; do
      i=/proc/$p/fd/0 o=/proc/$p/fd/1 e=/proc/$p/fd/2
      { : <"$i"; } 2>/dev/null || i=/dev/null
      { : >>"$o"; } 2>/dev/null || o=/dev/null
      { : >>"$e"; } 2>/dev/null || e=/dev/null
      eval "$c" <"$i" >>"$o" 2>>"$e"; echo "$?"
    done'
  }
  local w=${ROOTSH[1]} r=${ROOTSH[0]}
  exec {ROOT_W}>&"$w" {ROOT_R}<&"$r"
  # only the copies stay: closing them is the root shell's end of input
  eval "exec $w>&- $r<&-"
  SU=asroot
  # the password prompt happens here, on the terminal; a wrong one ends the root shell
  asroot true || { root_stop; SU=$tool; return 1; }
  case $tool in sudo) sudo -k ;; doas) doas -L 2>/dev/null ;; esac
  return 0
}
asroot() {
  local q rc me=$BASHPID
  [[ -n $ROOT_W ]] || return 1
  printf -v q '%q ' "$@"
  # the root shell opens this process's 0, 1 and 2 while it waits: a redirection here (`>&fd`,
  # `<&fd`) would swap them for that moment, so the request is written from a subshell and the
  # answer read with -u. A root shell that is gone must not take the installer with it (SIGPIPE).
  (trap '' PIPE; printf '%s\n%s\n' "$me" "$q" >&"$ROOT_W") 2>/dev/null || { echo "the root shell is gone" >&2; return 1; }
  IFS= read -r -u "$ROOT_R" rc || { echo "the root shell is gone" >&2; return 1; }
  return "$rc"
}
root_stop() {
  [[ -n $ROOT_W ]] && exec {ROOT_W}>&- {ROOT_R}<&-
  ROOT_W="" ROOT_R=""
}

# ── this machine: memory and threads decide how hard to build and how much to show ──
MEM_GB=$(awk '/^MemTotal:/{printf "%d", $2 / 1048576 + 0.5}' /proc/meminfo 2>/dev/null)
THREADS=$(nproc 2>/dev/null || echo 2)
[[ $MEM_GB =~ ^[0-9]+$ ]] || MEM_GB=8
# a light machine (an old laptop, a small VM): yami-niri (an hour of building there) is off by
# default, the packaged niri is the same desktop; the shell itself starts in lite mode (see
# `yami-shell config lite-auto`)
LIGHT=0
((MEM_GB < 6 || THREADS < 4)) && LIGHT=1
FORK_ON=$((1 - LIGHT))
[[ -n $FORK_FLAG ]] && FORK_ON=$FORK_FLAG
# rustc with fat LTO needs up to ~1.3 GB per job: on 4 GB four jobs swap for an hour, two finish
if [[ -z ${CARGO_BUILD_JOBS:-} ]]; then
  j=$((MEM_GB * 2 / 3))
  ((j < 1)) && j=1
  ((j > THREADS)) && j=$THREADS
  export CARGO_BUILD_JOBS=$j
fi

NVIDIA=0 HYBRID=0 RUNNING_NIRI=0 SYSTEMD=0
{ [[ -e /proc/driver/nvidia/version ]] || lspci 2>/dev/null | grep -qiE '(vga|3d).*nvidia'; } && NVIDIA=1
# NVIDIA next to another GPU (Optimus laptops, an iGPU driving a screen): the driver's global env
# (GBM, GLX, VA-API) would pull every app onto the NVIDIA card or break it, so it is left out
if ((NVIDIA)); then
  for v in /sys/class/drm/card[0-9]*/device/vendor; do
    [[ -r $v && $v != *-* ]] && [[ $(cat "$v") != 0x10de ]] && HYBRID=1
  done
fi
[[ -n "${NIRI_SOCKET:-}" ]] && RUNNING_NIRI=1
[[ -d /run/systemd/system ]] && SYSTEMD=1
# экран входа: по умолчанию — только если другого менеджера входа нет
OTHER_DM=""
if [[ -L /etc/systemd/system/display-manager.service ]]; then
  OTHER_DM="$(basename "$(readlink /etc/systemd/system/display-manager.service)" .service)"
  [[ $OTHER_DM == sddm ]] && OTHER_DM=""
fi
LOGIN_ON=1
[[ -n $OTHER_DM ]] && LOGIN_ON=0
[[ -n $LOGIN_FLAG ]] && LOGIN_ON=$LOGIN_FLAG
# игры: по умолчанию — только если здесь есть Steam (пакет, flatpak или его папка)
GAMES_ON=0
{ command -v steam >/dev/null || [[ -d $HOME/.local/share/Steam || -d $HOME/.steam || -d $HOME/.var/app/com.valvesoftware.Steam ]]; } && GAMES_ON=1
[[ -n $GAMES_FLAG ]] && GAMES_ON=$GAMES_FLAG

# ── packages by family ──────────────────────────────────────────────────────
# niri — the compositor; base — what the shell and the session use at runtime;
# build — to compile the shell; apps, look — optional; games — GameMode, MangoHud;
# tools — night light, text and QR from screenshots; nirisrc — to build niri itself
case "$FAM" in
  arch)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulse wireplumber libpulse networkmanager bluez bluez-utils
      cliphist wl-clipboard grim slurp brightnessctl playerctl python python-pillow noto-fonts noto-fonts-cjk noto-fonts-emoji
      power-profiles-daemon libxkbcommon pam fontconfig curl xz libcap)
    P_build=(rust gcc pkgconf libxkbcommon pam)
    P_apps=(kitty fastfetch pavucontrol btop udisks2 ffmpegthumbnailer libarchive mpv gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb)
    P_look=(qt6ct adw-gtk-theme papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract tesseract-data-eng tesseract-data-rus zbar)
    P_nirisrc=(git clang pkgconf libinput libxkbcommon mesa libdisplay-info seatd pango cairo pipewire systemd-libs) ;;
  fedora)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulseaudio wireplumber pulseaudio-utils NetworkManager bluez
      cliphist wl-clipboard grim slurp brightnessctl playerctl python3 python3-pillow google-noto-sans-fonts google-noto-sans-cjk-fonts
      google-noto-color-emoji-fonts tuned-ppd libxkbcommon pam fontconfig curl xz libcap)
    P_build=(rust cargo gcc pkgconf-pkg-config libxkbcommon-devel pam-devel)
    P_apps=(kitty fastfetch pavucontrol btop udisks2 ffmpegthumbnailer bsdtar mpv-libs gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb gvfs-fuse)
    P_look=(qt6ct adw-gtk3-theme papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract tesseract-langpack-eng tesseract-langpack-rus zbar)
    P_nirisrc=(git clang libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-devel systemd-devel
      libseat-devel pipewire-devel pango-devel cairo-gobject-devel libdisplay-info-devel) ;;
  debian)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulse wireplumber pulseaudio-utils network-manager bluez
      cliphist wl-clipboard grim slurp brightnessctl playerctl python3 python3-pil fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji
      power-profiles-daemon libxkbcommon0 libpam0g fontconfig curl xz-utils libcap2-bin)
    P_build=(cargo rustc gcc pkg-config libxkbcommon-dev libpam0g-dev)
    P_apps=(kitty fastfetch pavucontrol btop udisks2 ffmpegthumbnailer libarchive-tools libmpv2 gvfs gvfs-backends gvfs-fuse)
    P_look=(qt6ct adw-gtk3 papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-eng tesseract-ocr-rus zbar-tools)
    P_nirisrc=(git clang libudev-dev libgbm-dev libxkbcommon-dev libegl1-mesa-dev libwayland-dev libinput-dev libdbus-1-dev
      libsystemd-dev libseat-dev libpipewire-0.3-dev libpango1.0-dev libdisplay-info-dev) ;;
  suse)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulseaudio wireplumber pulseaudio-utils NetworkManager bluez
      cliphist wl-clipboard grim slurp brightnessctl playerctl python3 python3-Pillow google-noto-sans-fonts google-noto-sans-cjk-fonts
      noto-coloremoji-fonts power-profiles-daemon libxkbcommon0 pam fontconfig curl xz libcap-progs)
    P_build=(rust cargo gcc pkgconf libxkbcommon-devel pam-devel)
    P_apps=(kitty fastfetch pavucontrol btop udisks2 ffmpegthumbnailer bsdtar libmpv2 gvfs gvfs-backends gvfs-backend-afc gvfs-fuse)
    P_look=(qt6ct gtk3-metatheme-adwaita papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-traineddata-eng tesseract-ocr-traineddata-rus zbar)
    P_nirisrc=(git clang libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-1-devel systemd-devel
      libseat-devel pipewire-devel pango-devel cairo-devel libdisplay-info-devel) ;;
  void)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire wireplumber pulseaudio-utils NetworkManager bluez
      cliphist wl-clipboard grim slurp brightnessctl playerctl python3 python3-Pillow noto-fonts-ttf noto-fonts-cjk noto-fonts-emoji
      power-profiles-daemon libxkbcommon pam fontconfig curl xz libcap-progs)
    P_build=(rust cargo gcc pkg-config libxkbcommon-devel pam-devel)
    P_apps=(kitty fastfetch pavucontrol btop udisks2 ffmpegthumbnailer bsdtar mpv gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb)
    P_look=(qt6ct adw-gtk3 papirus-icon-theme)
    P_games=(gamemode MangoHud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-eng tesseract-ocr-rus zbar)
    P_nirisrc=(git clang eudev-libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-devel
      libseat-devel pipewire-devel pango-devel libdisplay-info-devel) ;;
  alpine)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulse wireplumber pulseaudio-utils networkmanager networkmanager-cli bluez
      cliphist wl-clipboard grim slurp brightnessctl playerctl python3 py3-pillow font-noto font-noto-cjk font-noto-emoji
      power-profiles-daemon libxkbcommon linux-pam fontconfig curl xz libcap-utils)
    P_build=(rust cargo gcc musl-dev pkgconf libxkbcommon-dev linux-pam-dev)
    P_apps=(kitty fastfetch pavucontrol btop udisks2 ffmpegthumbnailer libarchive-tools mpv-libs gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb gvfs-fuse)
    P_look=(qt6ct adw-gtk3 papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-data-rus zbar)
    P_nirisrc=(git clang eudev-dev mesa-dev libxkbcommon-dev wayland-dev libinput-dev dbus-dev libseat-dev pipewire-dev
      pango-dev libdisplay-info-dev) ;;
  gentoo)
    P_niri=(gui-wm/niri gui-apps/xwayland-satellite sys-apps/xdg-desktop-portal-gnome sys-apps/xdg-desktop-portal-gtk)
    P_base=(sys-auth/polkit gnome-base/gnome-keyring media-video/pipewire media-video/wireplumber
      net-misc/networkmanager net-wireless/bluez gui-apps/cliphist gui-apps/wl-clipboard gui-apps/grim gui-apps/slurp
      app-misc/brightnessctl media-sound/playerctl dev-python/pillow media-fonts/noto media-fonts/noto-cjk media-fonts/noto-emoji
      sys-power/power-profiles-daemon x11-libs/libxkbcommon sys-libs/pam net-misc/curl sys-libs/libcap)
    P_build=(virtual/rust virtual/pkgconfig)
    P_apps=(x11-terms/kitty app-misc/fastfetch media-sound/pavucontrol sys-process/btop sys-fs/udisks media-video/ffmpegthumbnailer app-arch/libarchive media-video/mpv gnome-base/gvfs)
    P_look=(gui-apps/qt6ct x11-themes/adw-gtk3 x11-themes/papirus-icon-theme)
    P_games=(games-util/gamemode games-util/mangohud)
    P_tools=(gui-apps/wlsunset app-text/tesseract media-gfx/zbar)
    P_nirisrc=(dev-vcs/git llvm-core/clang dev-libs/libinput sys-auth/seatd media-libs/mesa x11-libs/pango media-libs/libdisplay-info) ;;
  *)
    P_niri=() P_base=() P_build=() P_apps=() P_look=() P_games=() P_tools=() P_nirisrc=() ;;
esac
if ((PRINT_PKGS)); then
  # name: packages — tools/ci-distro.sh checks that each exists in this distro's repositories
  printf 'family: %s\n' "$FAM"
  for a in P_niri P_base P_build P_apps P_look P_games P_tools P_nirisrc; do
    declare -n arr=$a
    printf '%s: %s\n' "$a" "${arr[*]}"
  done
  exit 0
fi
NIX_PKGS="niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk polkit cliphist
wl-clipboard grim slurp brightnessctl playerctl pulseaudio (python3.withPackages (p: [ p.pillow ])) noto-fonts noto-fonts-cjk-sans
noto-fonts-color-emoji kitty fastfetch pavucontrol btop qt6ct adw-gtk3 papirus-icon-theme mpv
(services.power-profiles-daemon.enable = true; services.gvfs.enable = true;)"

# ── palettes (same as the shell) ────────────────────────────────────────────
PAL_ID=(blood ash sakura amber venom ice dusk)
PAL_RU=(кровь пепел сакура янтарь яд лёд сумрак)
PAL_EN=(blood ash sakura amber venom ice dusk)
PAL_HEX=(c4161c a8a29c d9577c d98e1f 5fae34 3a9fd0 8a5cf0)
pal=0
if [[ -n "$PALETTE" ]]; then
  for i in "${!PAL_ID[@]}"; do [[ ${PAL_ID[$i]} == "$PALETTE" ]] && pal=$i; done
elif [[ -f "$SET_JSON" ]]; then
  cur="$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("palette",""))' "$SET_JSON" 2>/dev/null)"
  for i in "${!PAL_ID[@]}"; do [[ ${PAL_ID[$i]} == "$cur" ]] && pal=$i; done
fi
# ── panel look: glass (panel opacity), lift (lighter graphite), inner (cards and graphs),
#    dim (behind menus); percent, written to the shell settings ────────────────────────
LOOK_ID=(matte glass light solid)
LOOK_RU=(матовое стекло светлое плотное)
LOOK_EN=(matte glass light solid)
LOOK_GLASS=(78 55 85 100) LOOK_LIFT=(0 20 60 0) LOOK_INNER=(100 55 80 100) LOOK_DIM=(50 35 30 60)
look=0 LOOK_SET=0
if [[ -n $LOOK_OPT ]]; then
  for i in "${!LOOK_ID[@]}"; do [[ ${LOOK_ID[$i]} == "$LOOK_OPT" ]] && { look=$i; LOOK_SET=1; }; done
  ((LOOK_SET)) || { echo "--look: ${LOOK_ID[*]}" >&2; exit 2; }
elif [[ -f "$SET_JSON" ]]; then
  cur="$(python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); print(round(d.get("glass",0.78)*100), round(d.get("lift",0)*100), round(d.get("inner",1)*100))' "$SET_JSON" 2>/dev/null)"
  for i in "${!LOOK_ID[@]}"; do [[ "${LOOK_GLASS[$i]} ${LOOK_LIFT[$i]} ${LOOK_INNER[$i]}" == "$cur" ]] && look=$i; done
fi
# ── theme: dark graphite (default) or light paper; the same palette, the whole rice follows ──
THEME_ID=(dark light)
theme=0 THEME_SET=0
if [[ -n $THEME_OPT ]]; then
  case $THEME_OPT in dark) theme=0 ;; light) theme=1 ;; *) echo "--theme: dark or light" >&2; exit 2 ;; esac
  THEME_SET=1
elif [[ -f "$SET_JSON" ]]; then
  python3 -I -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1])).get("light") else 1)' "$SET_JSON" 2>/dev/null && theme=1
fi
theme_name() { case ${THEME_ID[$1]} in dark) t "тёмная" "dark" ;; light) t "светлая" "light" ;; esac; }
theme_desc() {
  case ${THEME_ID[$1]} in
    dark) t "графит и акцент — как задумано" "graphite and the accent: the original look" ;;
    light) t "светлая бумага, акцент темнее; GTK, Qt, kitty и курсор — тоже" "light paper, a deeper accent; GTK, Qt, kitty and the cursor follow" ;;
  esac
}
# ── app font: readable sans (default) or the pixel Tiny5 everywhere; the shell stays pixel ──
FONT_ID=(sans pixel)
font=0
if [[ -n $FONT_OPT ]]; then
  case $FONT_OPT in sans) font=0 ;; pixel) font=1 ;; *) echo "--font: sans or pixel" >&2; exit 2 ;; esac
elif [[ -f "$SET_JSON" ]]; then
  cur="$(python3 -I -c 'import json,sys; d=json.load(open(sys.argv[1])); print("fontUi" in d and d["fontUi"] == "")' "$SET_JSON" 2>/dev/null)"
  # an older install without fontUi had the pixel font in apps
  [[ $cur == True ]] || python3 -I -c 'import json,sys; sys.exit("fontUi" in json.load(open(sys.argv[1])))' "$SET_JSON" 2>/dev/null && font=1
fi
# ── fastfetch: mini (default) or full with the hardware; an update keeps what is set ──
FETCH_ID=(mini full)
fetch=0 FETCH_SET=0
if [[ -n $FETCH_OPT ]]; then
  case $FETCH_OPT in mini) fetch=0 ;; full) fetch=1 ;; *) echo "--fetch: mini or full" >&2; exit 2 ;; esac
  FETCH_SET=1
elif [[ -f "$SET_JSON" ]]; then
  python3 -I -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1])).get("fetchFull") else 1)' "$SET_JSON" 2>/dev/null && fetch=1
fi
fetch_name() { case ${FETCH_ID[$1]} in mini) t "короткий" "mini" ;; full) t "расширенный" "extended" ;; esac; }
fetch_desc() {
  case ${FETCH_ID[$1]} in
    mini) t "кто, система, окружение — без железа" "who, system, desktop: no hardware" ;;
    full) t "ещё ядро, пакеты, процессор, видеокарта, память, диск, экран" "also kernel, packages, CPU, GPU, memory, disk, display" ;;
  esac
}
font_name() { case ${FONT_ID[$1]} in sans) t "обычный (Noto Sans)" "regular (Noto Sans)" ;; pixel) t "пиксельный (Tiny5)" "pixel (Tiny5)" ;; esac; }
font_desc() {
  case ${FONT_ID[$1]} in
    sans) t "программы читаются как обычно; шелл, файлы и экран входа — пиксельные" "apps read as usual; the shell, files and login screen stay pixel" ;;
    pixel) t "пиксельный шрифт во всех программах, ровно по сетке экрана" "the pixel font in every app, exactly on the screen's grid" ;;
  esac
}

look_name() { if [[ $L == ru ]]; then printf '%s' "${LOOK_RU[$1]}"; else printf '%s' "${LOOK_EN[$1]}"; fi; }
look_desc() {
  case ${LOOK_ID[$1]} in
    matte) t "тёмный графит, панели почти сплошные — как задумано" "dark graphite, nearly solid panels: the original look" ;;
    glass) t "панели и карточки прозрачные, стол просвечивает" "translucent panels and cards, the desktop shows through" ;;
    light) t "графит светлее, без глухого чёрного" "lighter graphite, no deep black" ;;
    solid) t "всё сплошное, сильное затемнение под меню" "fully solid, strong dim behind menus" ;;
  esac
}

pal_name() { if [[ $L == ru ]]; then printf '%s' "${PAL_RU[$1]}"; else printf '%s' "${PAL_EN[$1]}"; fi; }

# ── paint ───────────────────────────────────────────────────────────────────
rgb() { printf '%d;%d;%d' "0x${1:0:2}" "0x${1:2:2}" "0x${1:4:2}"; }
# accent mixed towards white (k>0) or black (k<0), in percent
shade() {
  local h=$1 k=$2 c out="" v
  for c in 0 2 4; do
    v=$((0x${h:c:2}))
    if ((k > 0)); then v=$((v + (255 - v) * k / 100)); else v=$((v * (100 + k) / 100)); fi
    out+="$v;"
  done
  printf '%s' "${out%;}"
}
paint() {
  [[ -t 1 ]] || { A="" AB="" AH="" AD=""; return; }
  local h=${PAL_HEX[$pal]}
  A=$'\e'"[38;2;$(rgb "$h")m"
  AB=$'\e'"[48;2;$(rgb "$h")m"
  AH=$'\e'"[38;2;$(shade "$h" 55)m"
  AD=$'\e'"[38;2;$(shade "$h" -55)m"
}
paint
T=$'\e[38;2;235;229;228m' D=$'\e[38;2;150;140;140m' F=$'\e[38;2;88;78;78m'
OK=$'\e[38;2;120;200;120m' WARN=$'\e[38;2;255;170;40m' BAD=$'\e[38;2;255;74;61m'
B=$'\e[1m' R=$'\e[0m'
[[ -t 1 ]] || T="" D="" F="" OK="" WARN="" BAD="" B="" R=""

pad() { printf '%s%*s' "$1" $(($2 - ${#1} > 0 ? $2 - ${#1} : 0)) ''; }
say() { printf "  %s▪%s %s%s%s\n" "$A" "$R" "$T" "$*" "$R"; }
note() { printf "    %s%s%s\n" "$F" "$*" "$R"; }
warn() { printf "  %s!%s %s%s%s\n" "$WARN" "$R" "$T" "$*" "$R"; }
nap() { ((ANIM)) && sleep "$1"; return 0; }

cleanup() { printf '\e[?25h%s' "$R"; }
trap cleanup EXIT
trap 'cleanup; echo; exit 130' INT
trap 'cleanup; exit 143' TERM
# the terminal may die mid-install (kitty reloads fonts and themes): finish the steps anyway
trap '' HUP

# ── banner: the logo dissolves in on a Bayer grid, then a light sweep ───────
LOGO1="▀▄▀ ▄▀█ █▀▄▀█ █"
LOGO2=" █  █▀█ █ ▀ █ █"
BAYER=(0 8 2 10 12 4 14 6 3 11 1 9 15 7 13 5)
logo_row() { # row text frame sweep
  local s=$2 f=$3 sw=$4 out="" i ch th
  for ((i = 0; i < ${#s}; i++)); do
    ch=${s:i:1}
    th=${BAYER[($1 % 4) * 4 + i % 4]}
    if [[ $ch == " " ]]; then out+=" "
    elif ((th < f - 2)); then
      if ((sw >= 0 && (i == sw || i == sw - 1))); then out+="$T$B$ch$R"
      elif ((sw >= 0 && i == sw - 2)); then out+="$AH$ch"
      else out+="$A$ch"; fi
    elif ((th < f)); then out+="$AD░"
    else out+=" "; fi
  done
  printf '  %s%s\e[K\n' "$out" "$R"
}
SUBTITLE=""
banner() { # 1 — animate
  SUBTITLE="$(t "пиксельный райс для niri" "pixel rice for niri")"
  local f sw
  printf '\n'
  if (($1)) && ((ANIM)); then
    printf '\e[?25l'
    for ((f = 0; f <= 18; f++)); do
      logo_row 0 "$LOGO1" "$f" -1
      logo_row 1 "$LOGO2" "$f" -1
      printf '\e[2A'
      sleep 0.022
    done
    for ((sw = 0; sw <= ${#LOGO1} + 3; sw += 2)); do
      logo_row 0 "$LOGO1" 99 "$sw"
      logo_row 1 "$LOGO2" 99 "$sw"
      printf '\e[2A'
      sleep 0.012
    done
  fi
  logo_row 0 "$LOGO1" 99 -1
  logo_row 1 "$LOGO2" 99 -1
  if (($1)) && ((ANIM)); then
    local line="────────────────────────────────────────" i
    printf '  %s' "$F"
    for ((i = 0; i < ${#line}; i += 4)); do printf '%s' "${line:i:4}"; sleep 0.008; done
    printf '%s\n  %s' "$R" "$D"
    for ((i = 0; i < ${#SUBTITLE}; i++)); do printf '%s' "${SUBTITLE:i:1}"; sleep 0.012; done
    printf '%s  %s// %s%s\n\n' "$R" "$F" "$(t "установка" "installer")" "$R"
  else
    printf '  %s────────────────────────────────────────%s\n' "$F" "$R"
    printf '  %s%s%s  %s// %s%s\n\n' "$D" "$SUBTITLE" "$R" "$F" "$(t "установка" "installer")" "$R"
  fi
}

# ── keys ────────────────────────────────────────────────────────────────────
key() {
  local k rest more
  # терминал закрыт (ssh оборвался, TTY ушёл): не крутиться в меню, а выйти
  IFS= read -rsn1 k || { echo eof; return; }
  if [[ $k == $'\e' ]]; then
    # по ssh и на медленном TTY хвост последовательности приходит позже: ждём дольше и
    # дочитываем всё (Home/End/F-клавиши длиннее двух байт), чтобы остаток не стал «клавишами»
    read -rsn2 -t 0.15 rest
    while IFS= read -rsn1 -t 0.01 more; do rest+=$more; done
    case "$rest" in "[A" | "OA") echo up ;; "[B" | "OB") echo down ;; "[C" | "OC") echo right ;; "[D" | "OD") echo left ;; "") echo esc ;; *) echo other ;; esac
  elif [[ $k == "" ]]; then echo enter
  elif [[ $k == " " ]]; then echo space
  else echo "$k"; fi
}
bye() { printf '\e[H\e[J  %s\n' "$(t "ну и ладно. ничего не тронуто." "ok, nothing was touched.")"; exit 0; }
screen() { printf '\e[H\e[J'; banner 0; }

# ── components ──────────────────────────────────────────────────────────────
C_ID=(core apps tools games look login nvidia fork)
C_ON=(1 1 "$TOOLS_FLAG" "$GAMES_ON" 1 "$LOGIN_ON" "$NVIDIA" "$FORK_ON")
# components this installer offered last time: one that came later (games, tools) starts at its
# default on an update instead of «off because it was not in the list»
KNOWN_F="$STATE/components-known"
# what was installed (the shell's `update stamp`): an update repeats the same choice
INSTALLED="$STATE/install.json"
COMP=""
if ((UPDATE)) && [[ -f $INSTALLED ]]; then
  COMP="$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("components",""))' "$INSTALLED" 2>/dev/null)"
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
c_name() {
  case $1 in
    core) t "niri и сеанс" "niri and session" ;;
    apps) t "программы" "apps" ;;
    tools) t "инструменты" "tools" ;;
    games) t "игры" "games" ;;
    look) t "тема программ" "app theme" ;;
    login) t "экран входа SDDM" "SDDM login screen" ;;
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
    login) if [[ -n $OTHER_DM ]]; then t "пиксельная тема как экран блокировки; заменит $OTHER_DM" "pixel theme like the lock screen; replaces $OTHER_DM"; else t "пиксельная тема как экран блокировки, вход в niri" "pixel theme like the lock screen, logs into niri"; fi ;;
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
# what was chosen, before this run drops what needs root: an update without sudo still knows
# which packages the chosen components want (do_need_pkgs)
C_WANT=("${C_ON[@]}")
wants() { local i; for i in "${!C_ID[@]}"; do [[ ${C_ID[$i]} == "$1" ]] && return $((1 - C_WANT[i])); done; return 1; }
for i in "${!C_ID[@]}"; do can "${C_ID[$i]}" || C_ON[i]=0; done
# yami-niri needs clang and the compositor's libraries: without root they must already be there
((SUDO)) || command -v clang >/dev/null || off fork

pick_lang() {
  local cur=$([[ $L == ru ]] && echo 0 || echo 1) opts=("Русский" "English") i mark style
  printf '\e[?25l'
  while :; do
    printf '\e[H\e[J'
    banner 0
    printf '  %s%slanguage · язык%s   %s↑↓ · enter%s\n\n' "$T" "$B" "$R" "$F" "$R"
    for i in 0 1; do
      if ((i == cur)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s\n' "$mark" "$style" "${opts[$i]}" "$R"
    done
    case "$(key)" in
      up | down | k | j | left | right) cur=$((1 - cur)) ;;
      enter) break ;;
      q | esc | eof) bye ;;
    esac
  done
  L=$([[ $cur == 0 ]] && echo ru || echo en)
}

pick_components() {
  local cur=0 n=${#C_ID[@]} i box mark style lock
  printf '\e[?25l'
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "что ставим" "components")" "$R" "$F" \
      "$(t "↑↓ выбор · пробел — галочка · a — всё · enter — дальше · q — выход" "↑↓ move · space toggle · a all · enter next · q quit")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((C_ON[i])); then box="${A}[■]${R}"; else box="${F}[ ]${R}"; fi
      if ((i == cur)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      lock=""
      can "${C_ID[$i]}" || lock=" $F($(t "нужен root и пакетный менеджер" "needs root and a package manager"))$R"
      printf '  %s %s %s%s%s%s\n' "$mark" "$box" "$style" "$(pad "$(c_name "${C_ID[$i]}")" 20)" "$R" "$lock"
      printf '        %s%s%s\n' "$F" "$(c_desc "${C_ID[$i]}")" "$R"
    done
    printf '\n  %s%s%s\n' "$F" "$(t "всегда: шелл, файлы, просмотр картинок (сборка из исходников), шрифты, пиксельный курсор, конфиги niri, kitty, fastfetch" \
      "always: the shell, files, image viewer (built from source), fonts, pixel cursor, niri, kitty and fastfetch configs")" "$R"
    printf '  %s%s: %s%s%s\n' "$F" "$(t "система" "system")" "$D" "$OS_NAME${PM:+ · $PM}" "$R"
    case "$(key)" in
      up | k) cur=$(((cur + n - 1) % n)) ;;
      down | j) cur=$(((cur + 1) % n)) ;;
      space) can "${C_ID[$cur]}" && C_ON[cur]=$((1 - C_ON[cur])) ;;
      a) for ((i = 0; i < n; i++)); do can "${C_ID[$i]}" && C_ON[i]=1; done ;;
      enter) return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

preview_bar() { # how the bar looks in this palette
  local bg=$'\e[48;2;14;12;12m' h=${PAL_HEX[$pal]} on=$'\e[38;2;255;255;255m'
  printf '  %s%s %s%s 1 %s%s %s2 3 %s│ %s■%s %s  %s%s %s■■■■%s■■■■%s 34%%  %s│  %s♪ ▂▅▇▃▆▂  %s17:42 %s\n' \
    "$bg" "$T" "$AB" "$on" "$R" "$bg" "$D" "$F" "$A" "$T" "$(t "стол" "desk")" "$D" "$(t "цп" "cpu")" "$A" "$F" "$D" "$F" "$A" "$T" "$R"
  printf '  '
  local k
  for k in -80 -65 -45 -25 0 25 45 65; do printf '\e[48;2;%sm    ' "$(shade "$h" "$k")"; done
  printf '%s\n' "$R"
}

pick_palette() {
  local n=${#PAL_ID[@]} i mark style
  while :; do
    paint
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "палитра" "palette")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом меняется в настройках (Mod+S)" "↑↓ move · enter next · change it later in settings (Mod+S)")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == pal)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s \e[48;2;%sm      %s  %s%s%s %s#%s%s\n' "$mark" "$(rgb "${PAL_HEX[$i]}")" "$R" "$style" "$(pad "$(pal_name "$i")" 8)" "$R" "$F" "${PAL_HEX[$i]}" "$R"
    done
    printf '\n'
    preview_bar
    case "$(key)" in
      up | k) pal=$(((pal + n - 1) % n)) ;;
      down | j) pal=$(((pal + 1) % n)) ;;
      enter) return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

# hex a → hex b by k percent, as hex
mixh() {
  local o="" c x y
  for c in 0 2 4; do
    x=$((0x${1:c:2})) y=$((0x${2:c:2}))
    o+="$(printf '%02x' $((x + (y - x) * $3 / 100)))"
  done
  printf '%s' "$o"
}
# a small panel over a striped "desktop": how dark, see-through and lifted it is
preview_look() {
  local i=$1 acc=${PAL_HEX[$pal]} wall1 wall2 light panel card bar row c j
  wall1="$(mixh 1a1416 "$acc" 30)" wall2="$(mixh 0c0a0a "$acc" 12)"
  light="$(mixh 4a4242 "$acc" 12)"
  panel="$(mixh 0e0c0c "$light" $((LOOK_LIFT[i] * 55 / 100)))"
  card="$(mixh 161313 "$light" $((LOOK_LIFT[i] * 55 / 100)))"
  for row in 0 1 2 3 4 5; do
    printf '  '
    for ((j = 0; j < 44; j++)); do
      c=$wall2
      (((j + row) % 6 < 3)) && c=$wall1
      if ((row >= 1 && row <= 4 && j >= 4 && j < 40)); then
        c="$(mixh "$c" "$panel" "${LOOK_GLASS[i]}")"
        if ((row >= 2 && row <= 3 && j >= 7 && j < 37)); then
          c="$(mixh "$c" "$card" "${LOOK_INNER[i]}")"
          ((row == 3 && j >= 9 && j < 27)) && c=$acc
        fi
        ((row == 1 && j >= 6 && j < 8)) && c=$acc
      fi
      printf '\e[48;2;%sm ' "$(rgb "$c")"
    done
    printf '%s\n' "$R"
  done
}

pick_look() {
  local n=${#LOOK_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "панели" "panels")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · тонко — в настройках → цвет" "↑↓ move · enter next · fine-tune in settings → color")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == look)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(look_name "$i")" 10)" "$R" "$F" "$(look_desc "$i")" "$R"
    done
    printf '\n'
    preview_look "$look"
    case "$(key)" in
      up | k) look=$(((look + n - 1) % n)) ;;
      down | j) look=$(((look + 1) % n)) ;;
      enter) LOOK_SET=1; return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

pick_font() {
  local n=${#FONT_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "шрифт программ" "app font")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом: настройки → шрифты" "↑↓ move · enter next · later: settings → fonts")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == font)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(font_name "$i")" 22)" "$R" "$F" "$(font_desc "$i")" "$R"
    done
    case "$(key)" in
      up | k | down | j) font=$((1 - font)) ;;
      enter) return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

pick_theme() {
  local n=${#THEME_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "тема" "theme")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом: настройки → цвет" "↑↓ move · enter next · later: settings → color")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == theme)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(theme_name "$i")" 22)" "$R" "$F" "$(theme_desc "$i")" "$R"
    done
    case "$(key)" in
      up | k | down | j) theme=$((1 - theme)) ;;
      enter) THEME_SET=1; return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

pick_fetch() {
  local n=${#FETCH_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "fastfetch" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом: настройки → вид" "↑↓ move · enter next · later: settings → look")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == fetch)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(fetch_name "$i")" 22)" "$R" "$F" "$(fetch_desc "$i")" "$R"
    done
    case "$(key)" in
      up | k | down | j) fetch=$((1 - fetch)) ;;
      enter) FETCH_SET=1; return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

confirm() {
  screen
  printf '  %s%s%s%s\n\n' "$T" "$B" "$(t "сводка" "summary")" "$R"
  local i
  printf '  %s■%s %s%s%s %s(%s)%s\n' "$A" "$R" "$T" "$OS_NAME" "$R" "$F" "${PM:-$(t "без пакетов" "no packages")}" "$R"
  for i in "${!C_ID[@]}"; do
    ((C_ON[i])) && printf '  %s■%s %s%s%s\n' "$A" "$R" "$T" "$(c_name "${C_ID[$i]}")" "$R"
  done
  printf '  %s■%s %s%s%s\n' "$A" "$R" "$T" "$(t "шелл, шрифты, niri, kitty, fastfetch" "shell, fonts, niri, kitty, fastfetch")" "$R"
  printf '  %s■%s %s%s: %s%s \e[48;2;%sm    %s\n' "$A" "$R" "$T" "$(t "палитра" "palette")" "$(pal_name "$pal")" "$R" "$(rgb "${PAL_HEX[$pal]}")" "$R"
  ((LOOK_SET)) && printf '  %s■%s %s%s: %s%s\n' "$A" "$R" "$T" "$(t "панели" "panels")" "$(look_name "$look")" "$R"
  printf '  %s■%s %s%s: %s%s\n' "$A" "$R" "$T" "$(t "тема" "theme")" "$(theme_name "$theme")" "$R"
  printf '  %s■%s %s%s: %s%s\n' "$A" "$R" "$T" "$(t "шрифт программ" "app font")" "$(font_name "$font")" "$R"
  printf '  %s■%s %sfastfetch: %s%s\n' "$A" "$R" "$T" "$(fetch_name "$fetch")" "$R"
  printf '  %s■%s %s%s%s\n\n' "$A" "$R" "$T" "$(t "язык: русский" "language: English")" "$R"
  [[ $FAM == nixos ]] && note "$(t "NixOS: пакеты добавь в configuration.nix — список будет в конце" "NixOS: add the packages to configuration.nix, the list is printed at the end")"
  ((ATOMIC)) && note "$(t "атомарная Fedora: пакеты ставятся слоем rpm-ostree, нужна перезагрузка" "atomic Fedora: packages are layered with rpm-ostree, a reboot is needed")"
  ((RUNNING_NIRI)) && note "$(t "niri уже запущен — шелл перезапустится сам, остальное подхватится на лету" "niri is running: the shell restarts by itself, the rest reloads live")"
  ((DRY)) && note "$(t "--dry-run: только план, ничего не меняется" "--dry-run: plan only, nothing changes")"
  printf '\n  %s%s%s %s[%s]%s ' "$T" "$(t "поехали?" "go?")" "$R" "$F" "$(t "enter — да, q — нет" "enter yes, q no")" "$R"
  printf '\e[?25h'
  case "$(key)" in enter | y) printf '\n' ;; *) bye ;; esac
}

# ── steps: spinner, elapsed time and a shimmering progress bar ──────────────
STEPS_T=() STEPS_F=()
add() { local title=$1; shift; STEPS_T+=("$title"); STEPS_F+=("$(printf '%q ' "$@")"); }
FAILED=() SKIPPED=()
SPIN=(⣾ ⣽ ⣻ ⢿ ⡿ ⣟ ⣯ ⣷)
progress() { # done total frame
  local w=32 fill=$(($1 * 32 / $2)) s="" j hl=$(($3 % 44 - 6))
  for ((j = 0; j < w; j++)); do
    if ((j < fill)); then
      if ((j == hl || j == hl + 1)); then s+="$T█"; elif ((j == hl - 1 || j == hl + 2)); then s+="$AH█"; else s+="$A█"; fi
    elif ((j == fill)); then s+="$AD▓"
    else s+="$F░"; fi
  done
  printf '  %s%s %s%d/%d%s' "$s" "$R" "$D" "$1" "$2" "$R"
}
elapsed() { local s=$1; ((s >= 60)) && printf '%dm%02ds' $((s / 60)) $((s % 60)) || printf '%ds' "$s"; }
step() { # index
  local i=$1 title=${STEPS_T[$1]} n=${#STEPS_T[@]} t0=$SECONDS rc f=0
  printf '\n### %s\n' "$title" >>"$LOG"
  if ((DRY)); then
    local cmd=${STEPS_F[$i]} room=$(($(tput cols 2>/dev/null || echo 100) - ${#title} - 8))
    ((${#cmd} > room)) && cmd="${cmd:0:room-1}…"
    printf '  %s·%s %s%s%s %s%s%s\n' "$A" "$R" "$T" "$title" "$R" "$F" "$cmd" "$R"
    return
  fi
  if [[ ! -t 1 || $SU == sudo || $SU == doas ]]; then
    # no terminal, or root without the shared root shell (its prompts must be seen): run in the
    # foreground; [k/n]: the shell's updates tab draws its progress from it
    printf '  %s▸%s %s[%d/%d] %s%s\n' "$A" "$R" "$T" $((i + 1)) "$n" "$title" "$R"
    eval "${STEPS_F[$i]}" 2>&1 | tee -a "$LOG" | sed 's/^/      /'
    rc=${PIPESTATUS[0]}
  else
    # root commands go to the root shell authorised before the steps (root_start): nothing here
    # asks for a password, so every step can run behind the spinner
    (eval "${STEPS_F[$i]}") >>"$LOG" 2>&1 </dev/null &
    local pid=$!
    printf '\e[?25l'
    while kill -0 "$pid" 2>/dev/null; do
      printf '\r\e[K  %s%s%s %s%s%s %s%s%s\n\e[K' "$A" "${SPIN[f % 8]}" "$R" "$T" "$title" "$R" "$F" "$(elapsed $((SECONDS - t0)))" "$R"
      progress "$i" "$n" "$f"
      printf '\e[1A\r'
      ((f++))
      sleep 0.07
    done
    wait "$pid"
    rc=$?
  fi
  local dt
  dt="$(elapsed $((SECONDS - t0)))"
  if ((rc == 0)); then
    printf '\r\e[K  %s✓%s %s%s%s %s%s%s\n\e[K' "$OK" "$R" "$T" "$title" "$R" "$F" "$dt" "$R"
  elif ((rc == 99)); then
    printf '\r\e[K  %s–%s %s%s%s %s%s%s\n\e[K' "$F" "$R" "$D" "$title" "$R" "$F" "$(t "не нужно" "not needed")" "$R"
    SKIPPED+=("$title")
  else
    printf '\r\e[K  %s✗%s %s%s%s %s— %s%s\n\e[K' "$BAD" "$R" "$T" "$title" "$R" "$F" "$(t "хвост лога:" "log tail:")" "$R"
    tail -n 6 "$LOG" | sed "s/^/      $F/; s/\$/$R/"
    FAILED+=("$title")
  fi
  printf '\e[?25h'
}
run_steps() {
  local i
  for ((i = 0; i < ${#STEPS_T[@]}; i++)); do
    step "$i"
    # an update whose build failed stops here: the old binaries stay with the configs they match,
    # and the install record keeps the old version, so the update is offered again
    if ((UPDATE)) && [[ ${STEPS_F[$i]%% *} == do_build_shell ]] && ((${#FAILED[@]})); then
      warn "$(t "сборка не удалась — обновление остановлено, стоит прежняя версия" "the build failed: update stopped, the previous version stays")"
      break
    fi
  done
  if [[ -t 1 ]] && ((!DRY)); then
    local f
    for ((f = 0; f < 22; f++)); do printf '\r'; progress "${#STEPS_T[@]}" "${#STEPS_T[@]}" $((f * 2)); nap 0.02; done
    printf '\n'
  fi
}

backup() { [[ -e "$1" && ! -L "$1" ]] && mv -- "$1" "$1.bak-$STAMP"; return 0; }

# ── package manager ─────────────────────────────────────────────────────────
pm_refresh() {
  case $PM in
    apt) $SU apt-get update ;;
    zypper) $SU zypper --non-interactive refresh ;;
    xbps) $SU xbps-install -Sy ;;
    apk) $SU apk update ;;
    *) true ;;
  esac
}
pm_install() {
  case $PM in
    pacman) $SU pacman -S --needed --noconfirm "$@" ;;
    dnf) $SU dnf install -y "$@" ;;
    rpm-ostree) $SU rpm-ostree install --idempotent --allow-inactive "$@" ;;
    apt) $SU env DEBIAN_FRONTEND=noninteractive apt-get install -y "$@" ;;
    zypper) $SU zypper --non-interactive install --no-recommends "$@" ;;
    xbps) $SU xbps-install -y "$@" ;;
    apk) $SU apk add "$@" ;;
    emerge) $SU emerge --noreplace --quiet-build "$@" ;;
    *) return 1 ;;
  esac
}
# whole list at once; if the batch fails (a name this distro lacks), one by one
pkgs() {
  (($#)) || return 0
  pm_install "$@" && return 0
  # usually one name this release does not have (Alpine: cliphist, qt6ct): drop what the
  # repositories lack and try the rest at once again; one by one only if that fails too
  local p have=()
  if pkg_avail_known; then
    for p; do
      if pkg_avail "$p"; then have+=("$p"); else echo "$p" >>"$MISSING_F"; echo "not in the repositories: $p"; fi
    done
    ((${#have[@]})) || return 0
    pm_install "${have[@]}" && return 0
  else
    have=("$@")
  fi
  echo "batch failed, installing one by one"
  for p in "${have[@]}"; do pm_install "$p" || echo "$p" >>"$MISSING_F"; done
  return 0
}

# ── shell build ─────────────────────────────────────────────────────────────
ver_ge() { [[ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" == "$2" ]]; }
rust_ok() {
  local v
  v="$(rustc --version 2>/dev/null | awk '{print $2}')"
  [[ -n $v ]] && ver_ge "$v" "$RUST_MIN" && command -v cargo >/dev/null
}
need_rust() {
  [[ -d $HOME/.cargo/bin ]] && export PATH="$HOME/.cargo/bin:$PATH"
  rust_ok && return 0
  [[ $FAM == nixos ]] && return 0 # nix-shell provides it
  if command -v rustup >/dev/null; then
    rustup toolchain install stable --profile minimal && rustup default stable
  else
    echo "rustc older than $RUST_MIN or missing: installing rustup into ~/.cargo"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path
  fi
  export PATH="$HOME/.cargo/bin:$PATH"
  rust_ok
}
do_build_shell() {
  need_rust || return 1
  local cmd=(cargo build --release --locked --workspace --manifest-path "$ROOT/shell/Cargo.toml" --target-dir "$CACHE/target")
  # a weak machine: thin LTO in 16 units instead of fat LTO in one. Measured with two jobs: the
  # rustc peak is 1.3 GB instead of 2.4 GB (fat LTO links each binary with all its crates at once),
  # so 4 GB does not swap; the binaries are ~20 % larger, the speed difference is not visible
  ((LIGHT)) && cmd=(env CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 "${cmd[@]}")
  if [[ $FAM == nixos ]] && ! rust_ok; then
    nix-shell -p cargo rustc pkg-config libxkbcommon pam --run "${cmd[*]}" || return 1
  else
    "${cmd[@]}" || return 1
  fi
  install -Dm755 "$CACHE/target/release/yami-shell" "$BIN/yami-shell"
  install -Dm755 "$CACHE/target/release/yami-files" "$BIN/yami-files"
  install -Dm755 "$CACHE/target/release/yami-view" "$BIN/yami-view"
  install -Dm755 "$CACHE/target/release/yami-play" "$BIN/yami-play"
  # `yami <command>`: the same binary, help instead of the shell when started without a command
  ln -sfn yami-shell "$BIN/yami"
}

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
  # a portal backend left from the previous build would spawn a deleted binary
  pgrep -f '^[^ ]*yami-files --pick' >/dev/null || pkill -f '^[^ ]*yami-files --portal' 2>/dev/null
  if ((UPDATE)); then
    # update: only our backend comes back (new binary); restarting the portal frontend would cut
    # a running screencast (OBS, Discord)
    busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus StartServiceByName su \
      org.freedesktop.impl.portal.desktop.yami 0 >/dev/null 2>&1
  elif ((SYSTEMD)) && systemctl --user is-active -q xdg-desktop-portal 2>/dev/null; then
    busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ReloadConfig >/dev/null 2>&1
    systemctl --user restart xdg-desktop-portal
  fi
  return 0
}

# ── niri from upstream source, only when no package provided it ────────────
niri_ok() {
  local v
  v="$(niri --version 2>/dev/null | awk '{print $2}')"
  [[ -n $v ]] && ver_ge "$v" "$NIRI_MIN"
}
do_niri_source() {
  niri_ok && return 99
  [[ -n $SU || $EUID == 0 ]] || return 1
  need_rust || return 1
  local src="$CACHE/niri-$NIRI_TAG"
  [[ -d $src/.git ]] || git clone --depth 1 --branch "$NIRI_TAG" https://github.com/YaLTeR/niri.git "$src" || return 1
  cargo build --release --locked --manifest-path "$src/Cargo.toml" --target-dir "$CACHE/target-niri" || return 1
  $SU install -Dm755 "$CACHE/target-niri/release/niri" /usr/local/bin/niri
  $SU install -Dm755 "$src/resources/niri-session" /usr/local/bin/niri-session
  $SU install -Dm644 "$src/resources/niri.desktop" /usr/share/wayland-sessions/niri.desktop
  $SU install -Dm644 "$src/resources/niri-portals.conf" /usr/local/share/xdg-desktop-portal/niri-portals.conf
  if ((SYSTEMD)); then
    sed 's|/usr/bin/niri|/usr/local/bin/niri|g' "$src/resources/niri.service" | $SU tee /etc/systemd/user/niri.service >/dev/null
    $SU install -Dm644 "$src/resources/niri-shutdown.target" /etc/systemd/user/niri-shutdown.target
  fi
}

# ── fonts ───────────────────────────────────────────────────────────────────
do_fonts() {
  local dir="$HOME/.local/share/fonts/yami"
  rm -f -- "$dir/Handjet.ttf" "$dir/OFL-handjet.txt"   # dropped: never used
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
  # old blur rule covered whole shell surfaces (dark frames around panels); the shell now sends
  # exact blur regions itself and rewrites this file on start
  grep -qs 'yami-(bar' "$n/yami-fx.kdl" && rm -f "$n/yami-fx.kdl"
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
# update: yami-niri only when the patch series moved (5–25 min), else the built one stays
fork_stale() {
  local want have
  want="$(sed -n 's/^VERSION="\(.*\)"/\1/p' "$ROOT/fork/yami-niri.sh")"
  have="$("$HOME/.local/lib/yami-niri/current/niri" --version 2>/dev/null | awk '{print $2}')"
  [[ $want != "$have" ]]
}
# update: the installed login theme differs from this tree (its QML; fonts and colours are added on install)
sddm_stale() {
  local f
  for f in "$ROOT"/sddm/yami/*.qml; do cmp -s "$f" "/usr/share/sddm/themes/yami/${f##*/}" || return 0; done
  return 1
}
# the yami-niri sources and their build cache are ~4 GB and only the next fork build needs them (the
# installed binary does not), so after a successful build they go; the next build clones them again.
# Kept with --keep-build / YAMI_KEEP_BUILD, when this is not the updater's own clone (running from
# any other checkout is development), and when the trees hold work: edits or commits not from the
# patch series (those are made by yami@localhost, `git am` in yami-niri.sh)
# the trees hold only what yami-niri.sh makes (no other folder by mistake via YAMI_NIRI_SRC) and no
# work: no edits, no commits outside the patch series (those are made by yami@localhost, `git am`)
niri_src_plain() {
  local src=$1 e d
  [[ -d $src ]] || return 1
  for e in "$src"/* "$src"/.[!.]*; do
    [[ -e $e ]] || continue
    case ${e##*/} in yami-niri | yami-smithay) ;; *) return 1 ;; esac
  done
  for d in "$src/yami-niri" "$src/yami-smithay"; do
    [[ -d $d/.git ]] || continue
    [[ -z $(git -C "$d" status --porcelain 2>/dev/null) ]] || return 1
    ! git -C "$d" log --format=%ce HEAD --not --remotes 2>/dev/null | grep -qvx 'yami@localhost' || return 1
  done
}
fork_tidy() {
  ((KEEP_BUILD)) || [[ -n ${YAMI_KEEP_BUILD:-} ]] && return 0
  local src="${YAMI_NIRI_SRC:-$CACHE/niri-src}" sz
  [[ $ROOT == "${XDG_DATA_HOME:-$HOME/.local/share}/yami/src" ]] && niri_src_plain "$src" || return 0
  sz="$(du -sh "$src" 2>/dev/null | cut -f1)"
  rm -rf -- "$src" && note "$(t "исходники yami-niri удалены (${sz:-?}): бинарник стоит, следующая сборка скачает их заново (--keep-build — оставить)" "yami-niri sources removed (${sz:-?}): the binary stays, the next build fetches them again (--keep-build keeps them)")"
}

# the install counter: one download of a tiny release file per machine, ever. GitHub counts
# downloads of release files; the request carries nothing about the machine (no id, no version, no
# data: what any download from GitHub sends) and the file is thrown away. --no-count,
# YAMI_NO_COUNT=1 or CI skip it. A marker in the state dir keeps it to one time per machine
COUNT_URL="https://github.com/Trendorin/yami/releases/latest/download/yami-installs.txt"
do_count() {
  local mark="${XDG_STATE_HOME:-$HOME/.local/state}/yami/counted"
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

# ── packages the chosen components want but the system lacks ───────────────
# An update runs without sudo: it cannot install what a new version needs (night light, OCR,
# GameMode…). It writes the package manager and the names to $STATE/packages-needed; `yami doctor`
# and settings → updates show one command and run it in a terminal (the password stays there).
# Only names the repositories have: one that is not packaged here is no use in the command.
pkg_has() {
  case $PM in
    pacman) pacman -T "$1" >/dev/null 2>&1 ;;
    dnf | rpm-ostree | zypper) rpm -q --whatprovides "$1" >/dev/null 2>&1 ;;
    apt) dpkg-query -W -f='${Status}' "$1" 2>/dev/null | grep -q "install ok installed" ;;
    xbps) xbps-query "$1" >/dev/null 2>&1 ;;
    apk) apk info -e "$1" >/dev/null 2>&1 ;;
    *) return 0 ;;
  esac
}
# pkg_avail can answer for this package manager (Gentoo and atomic Fedora: not without root/network)
pkg_avail_known() { [[ $PM == pacman || $PM == dnf || $PM == apt || $PM == zypper || $PM == xbps || $PM == apk ]]; }
pkg_avail() {
  case $PM in
    pacman) pacman -Si "$1" >/dev/null 2>&1 ;;
    dnf) dnf -q -C info "$1" >/dev/null 2>&1 || dnf -q info "$1" >/dev/null 2>&1 ;;
    apt) [[ -n "$(apt-cache policy "$1" 2>/dev/null | sed -n 's/^ *Candidate: //p' | grep -v '(none)')" ]] ;;
    zypper) zypper --no-refresh -q se -x "$1" >/dev/null 2>&1 ;;
    xbps) xbps-query -R "$1" >/dev/null 2>&1 ;;
    apk) [[ -n "$(apk search -e "$1" 2>/dev/null)" ]] ;;
    rpm-ostree) return 0 ;;
    *) return 1 ;;
  esac
}
do_need_pkgs() {
  local f="$STATE/packages-needed" want=() need=() p
  wants core && want+=("${P_base[@]}")
  wants apps && want+=("${P_apps[@]}")
  wants tools && want+=("${P_tools[@]}")
  wants games && want+=("${P_games[@]}")
  wants look && want+=("${P_look[@]}")
  [[ -n $PM && $PM != nix && $PM != emerge ]] || { rm -f "$f"; return 99; }
  for p in "${want[@]}"; do
    pkg_has "$p" && continue
    grep -qx -- "$p" "$MISSING_F" 2>/dev/null && continue
    pkg_avail "$p" && need+=("$p")
  done
  if ((${#need[@]})); then
    mkdir -p "$STATE"
    printf 'pm %s\npkgs %s\n' "$PM" "${need[*]}" >"$f"
    echo "missing: ${need[*]}"
  else
    rm -f "$f"
  fi
}

# ── login screen: SDDM with the yami theme ──────────────────────────────────
do_sddm() {
  local dst=/usr/share/sddm/themes/yami src="$CACHE/sddm-yami"
  rm -rf -- "$src"
  "$ROOT/tools/sddm-build.sh" "$src" >/dev/null || return 1
  $SU rm -rf -- "$dst"
  $SU mkdir -p "$(dirname "$dst")"
  $SU cp -r "$src" "$dst" || return 1
  # палитру и обои при смене цвета переписывает tools/theme.py — от тебя, без sudo
  $SU touch "$dst/theme.conf.user" "$dst/background.png"
  $SU chown "$(id -u):$(id -g)" "$dst/theme.conf.user" "$dst/background.png"
  $SU chmod 644 "$dst/theme.conf.user" "$dst/background.png"
  python3 -I "$ROOT/tools/theme.py" --sddm "$dst"
  $SU mkdir -p /etc/sddm.conf.d
  # the pixel cursor on the login screen too: the greeter reads /usr/share/icons only
  python3 -I "$ROOT/tools/theme.py" --cursor-to "$CACHE/yami-cursor" || return 1
  $SU rm -rf -- /usr/share/icons/yami-cursor
  $SU cp -r "$CACHE/yami-cursor" /usr/share/icons/yami-cursor || return 1
  rm -rf -- "$CACHE/yami-cursor"
  printf '%s\n' "# yami: login screen theme (installer)" "[Theme]" "Current=yami" "CursorTheme=yami-cursor" "CursorSize=24" \
    "" "[Users]" "RememberLastUser=true" "RememberLastSession=true" | $SU tee /etc/sddm.conf.d/10-yami.conf >/dev/null
  # first boot: this user and the Niri session already picked (SDDM keeps its own choice later)
  local st=/var/lib/sddm/state.conf sess
  sess="$(ls /usr/share/wayland-sessions/niri.desktop /usr/local/share/wayland-sessions/niri.desktop 2>/dev/null | head -n1)"
  if [[ -n $sess && -d ${st%/*} ]] && ! $SU grep -qs '^Session=' "$st"; then
    printf '[Last]\nSession=%s\nUser=%s\n' "$sess" "$(id -un)" | $SU tee "$st" >/dev/null
    $SU chown sddm:sddm "$st" 2>/dev/null
  fi
  if ((SYSTEMD)) && [[ -f /usr/lib/systemd/system/plymouth-quit.service ]]; then
    # зависший plymouth не должен держать multi-user.target (system/plymouth)
    local u
    for u in plymouth-quit plymouth-quit-wait; do
      $SU install -Dm644 "$ROOT/system/plymouth/10-yami.conf" "/etc/systemd/system/$u.service.d/10-yami.conf"
    done
    $SU systemctl daemon-reload
  fi
  if ((SYSTEMD)); then
    # заменить текущий менеджер входа — со следующей загрузки, сеанс не трогаем
    $SU systemctl enable -f sddm.service || return 1
  fi
}

do_nvidia_profile() {
  $SU install -Dm644 "$ROOT/system/nvidia/50-niri-vram.json" \
    /etc/nvidia/nvidia-application-profiles-rc.d/50-niri-vram.json
}

# NVIDIA early KMS: without the driver in the initramfs the LUKS prompt and the splash only show
# where the firmware lit a picture (often HDMI only, a black DisplayPort)
NV_MODS="nvidia nvidia_modeset nvidia_uvm nvidia_drm"
do_nvidia_early() {
  modinfo nvidia_drm >/dev/null 2>&1 || return 99
  # a hybrid laptop shows its panel through the other GPU: nothing to gain, and the driver would load early
  ((HYBRID)) && return 99
  $SU install -Dm644 "$ROOT/system/nvidia/yami-nvidia-kms.conf" /etc/modprobe.d/yami-nvidia-kms.conf || return 1
  if [[ -x /usr/bin/mkinitcpio ]]; then
    grep -qs '^[^#]*MODULES.*nvidia_drm' /etc/mkinitcpio.conf /etc/mkinitcpio.conf.d/*.conf && return 0
    printf '# yami: NVIDIA in the initramfs, a picture at the disk password prompt on any output\nMODULES+=(%s)\n' "$NV_MODS" |
      $SU tee /etc/mkinitcpio.conf.d/90-yami-nvidia.conf >/dev/null || return 1
    # CachyOS/Limine wrap mkinitcpio so that boot entries are updated too
    if command -v limine-mkinitcpio >/dev/null; then $SU limine-mkinitcpio; else $SU /usr/bin/mkinitcpio -P; fi
  elif command -v dracut >/dev/null; then
    printf '# yami: NVIDIA in the initramfs\nadd_drivers+=" %s "\ninstall_items+=" /etc/modprobe.d/yami-nvidia-kms.conf "\n' "$NV_MODS" |
      $SU tee /etc/dracut.conf.d/90-yami-nvidia.conf >/dev/null || return 1
    $SU dracut -f --regenerate-all
  else
    return 99
  fi
}

# OBS on Pascal and older: OBS and ffmpeg want NVENC API 13.1 (driver 610+), these cards end on
# the 580 branch (13.0), so OBS lists no NVENC at all. GStreamer's nvcodec works with any driver;
# obs-gstreamer brings it into OBS as «GStreamer Encoder» (nvh264enc, nvh265enc)
do_nvidia_obs() {
  command -v obs-nvenc-test >/dev/null || return 99
  obs-nvenc-test 2>/dev/null | grep -q '^reason=outdated_driver' || return 99
  [[ $PM == pacman ]] && pacman -Si obs-gstreamer >/dev/null 2>&1 || return 99
  pkgs obs-gstreamer gst-plugins-bad
}

# the old fork (dark-niri on angel-niri): its build, drop-in and script copy
do_drop_dark_niri() {
  local d="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/niri.service.d/dark-niri.conf"
  [[ -e $d || -d $HOME/.local/lib/dark-niri || -d $HOME/.local/share/yami/fork ]] || return 99
  rm -f -- "$d"
  [[ "$(readlink "$BIN/niri" 2>/dev/null)" == "$HOME/.local/lib/dark-niri"/* ]] && rm -f -- "$BIN/niri"
  rm -rf -- "$HOME/.local/lib/dark-niri" "$HOME/.local/share/yami/fork"
  ((SYSTEMD)) && systemctl --user daemon-reload 2>/dev/null
  return 0
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

# ── moving from the old name (DARK-4RMY) ────────────────────────────────────
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

# ── uninstall ───────────────────────────────────────────────────────────────
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
  rm -f "$STATE/packages-needed" "$STATE/components-known" "$STATE/settings-newer.json"
  # the build cache (gigabytes); yami-niri sources only without patch work in them
  rm -rf -- "$CACHE/target" "$CACHE/sddm-yami" "$CACHE/update.log" "$CACHE/update.lock"
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

# ── go ──────────────────────────────────────────────────────────────────────
((UNINSTALL)) && uninstall

if ((UPDATE)); then
  # rebuild from this tree with the choices of the first install; no packages, no sudo
  say "$(t "обновление из" "update from") $ROOT"
  add "$(t "шелл: сборка из исходников" "shell: build from source")" do_build_shell
  add "$(t "файлы: yami-files и диалог выбора файлов" "files: yami-files and the file dialog")" do_files
  add "$(t "шрифты" "fonts")" do_fonts
  add "$(t "niri: конфиг" "niri: config")" do_niri_config
  add "$(t "палитра: всё, что из неё выводится" "palette: everything derived from it")" do_refresh
  add "$(t "запись об установке" "install record")" do_stamp
  # an update has no sudo: what the chosen components still miss goes to packages-needed, and the
  # shell offers one command for it (settings → updates, yami doctor, yami update need)
  add "$(t "каких пакетов ещё не хватает" "packages still missing")" do_need_pkgs
  add "$(t "счётчик установок (один раз, без данных)" "install counter (once, no data)")" do_count
  ((RUNNING_NIRI)) && add "$(t "перезапуск шелла" "restart the shell")" do_restart_shell
  run_steps
  if on fork && ((!DRY && ${#FAILED[@]} == 0)) && fork_stale; then
    say "$(t "yami-niri: новая серия патчей — сборка (5–25 минут)" "yami-niri: a new patch series, building (5–25 minutes)")"
    if need_rust && "$ROOT/fork/yami-niri.sh" build 2>&1 | tee -a "$LOG"; then fork_tidy; else FAILED+=("yami-niri"); fi
  fi
  if on login && [[ -d /usr/share/sddm/themes/yami ]] && sddm_stale; then
    note "$(t "экран входа обновился — поставь его: $ROOT/tools/sddm-install.sh (нужен sudo)" "the login screen changed, install it: $ROOT/tools/sddm-install.sh (needs sudo)")"
  fi
  if ((${#FAILED[@]})); then
    warn "$(t "не вышло:" "failed:") ${FAILED[*]}"
    note "$(t "подробности:" "details:") $LOG"
    exit 1
  fi
  say "$(t "обновлено" "updated")"
  exit 0
fi

if ((!YES)); then
  printf '\e[H\e[J'
  banner 1
  ((LANG_ASK)) && pick_lang
  pick_components
  C_WANT=("${C_ON[@]}")
  pick_palette
  pick_look
  pick_theme
  pick_font
  pick_fetch
  confirm
fi
paint
[[ -t 1 ]] && printf '\e[H\e[J'
banner $((YES))

if ((SUDO && !DRY)) && [[ -n $SU ]]; then
  say "$(t "нужен пароль администратора — один раз на всю установку" "the administrator password, once for the whole install")"
  if root_start; then
    :
  elif $SU true; then
    # the password is right but the root shell did not come up (an unusual sudo setup): every root
    # command asks on its own, the steps run in the foreground so the prompts are seen
    warn "$(t "общий root-сеанс не поднялся — пароль может спрашиваться не раз" "the shared root session did not start: the password may be asked more than once")"
  else
    warn "$(t "без root — ставлю только домашнюю папку" "no root: home directory only")"
    SUDO=0
    for i in "${!C_ID[@]}"; do can "${C_ID[$i]}" || C_ON[i]=0; done
  fi
  printf '\n'
fi

if ((SUDO)) && { on core || on apps || on look; }; then
  add "$(t "пакеты: обновить списки" "packages: refresh index")" pm_refresh
fi
on core && add "$(t "пакеты: niri" "packages: niri")" pkgs "${P_niri[@]}"
on core && add "$(t "пакеты: сеанс и шелл" "packages: session and shell")" pkgs "${P_base[@]}"
# a working toolchain (rustup included) is kept: only the libraries and pkg-config are added
P_build_need=()
for p in "${P_build[@]}"; do
  case "$p" in rust | rustc | cargo | virtual/rust) rust_ok && continue ;; esac
  P_build_need+=("$p")
done
((SUDO)) && add "$(t "пакеты: сборка шелла" "packages: shell build deps")" pkgs "${P_build_need[@]}"
on apps && add "$(t "пакеты: программы" "packages: apps")" pkgs "${P_apps[@]}"
on look && add "$(t "пакеты: тема Qt и GTK" "packages: Qt and GTK theme")" pkgs "${P_look[@]}"
on tools && add "$(t "пакеты: ночной свет, текст и QR со скриншота" "packages: night light, text and QR from screenshots")" pkgs "${P_tools[@]}"
on games && add "$(t "пакеты: GameMode, MangoHud" "packages: GameMode, MangoHud")" pkgs "${P_games[@]}"
on login && add "$(t "пакеты: SDDM" "packages: SDDM")" pkgs "$([[ $FAM == gentoo ]] && echo x11-misc/sddm || echo sddm)"
if on core; then
  niri_ok || ((DRY)) || add "$(t "пакеты: сборка niri" "packages: niri build deps")" pkgs "${P_nirisrc[@]}"
  add "$(t "niri $NIRI_MIN+ из исходников, если пакет старее" "niri $NIRI_MIN+ from source if the package is older")" do_niri_source
fi
on nvidia && add "$(t "NVIDIA: профиль против утечки VRAM" "NVIDIA: VRAM leak profile")" do_nvidia_profile
on nvidia && add "$(t "NVIDIA: картинка с начала загрузки (пароль диска на любом выходе)" "NVIDIA: picture from the start of boot (disk password on any output)")" do_nvidia_early
on nvidia && add "$(t "NVIDIA: кодек видеокарты в OBS на старом драйвере (GStreamer)" "NVIDIA: the GPU encoder in OBS on an old driver (GStreamer)")" do_nvidia_obs
on core && ((SUDO)) && add "$(t "службы: bluetooth, профили питания, сеть" "services: bluetooth, power profiles, network")" do_services
on fork && ((SUDO)) && add "$(t "пакеты: сборка yami-niri" "packages: yami-niri build deps")" pkgs "${P_nirisrc[@]}"
old_install && add "$(t "переезд со старого имени DARK-4RMY" "move from the old name DARK-4RMY")" do_migrate
add "$(t "шелл: сборка из исходников" "shell: build from source")" do_build_shell
add "$(t "файлы: yami-files и диалог выбора файлов" "files: yami-files and the file dialog")" do_files
add "$(t "шрифты: Tiny5, DotGothic16, Terminess, Monocraft" "fonts: Tiny5, DotGothic16, Terminess, Monocraft")" do_fonts
add "$(t "niri: конфиг и настройки под железо" "niri: config and hardware settings")" do_niri_config
add "$(t "палитра «$(pal_name "$pal")»: шелл, kitty, fastfetch, обои" "palette «$(pal_name "$pal")»: shell, kitty, fastfetch, wallpaper")" do_dotfiles
on look && add "$(t "тема программ: Qt, KDE, GTK" "app theme: Qt, KDE, GTK")" do_look
on login && add "$(t "экран входа: тема yami для SDDM" "login screen: yami theme for SDDM")" do_sddm
add "$(t "убрать старый форк dark-niri" "remove the old dark-niri fork")" do_drop_dark_niri
old_install && add "$(t "убрать файлы под старым именем" "remove files under the old name")" do_drop_old
add "$(t "запись об установке (для yami update)" "install record (for yami update)")" do_stamp
add "$(t "каких пакетов ещё не хватает" "packages still missing")" do_need_pkgs
add "$(t "счётчик установок (один раз, без данных)" "install counter (once, no data)")" do_count
((RUNNING_NIRI)) && add "$(t "перезапуск шелла" "restart the shell")" do_restart_shell

run_steps

if on fork && ((!DRY)); then
  printf '\n  %s%s%s\n\n' "$T" "$(t "сборка yami-niri — 5–25 минут, вывод прямо здесь:" "building yami-niri takes 5–25 minutes, output below:")" "$R"
  if need_rust && "$ROOT/fork/yami-niri.sh" build 2>&1 | tee -a "$LOG"; then
    # realtime priority without the rtprio limit: the capability on the binary (dropped at start)
    if [[ "$(ulimit -r)" == 0 ]] && ((SUDO)) && command -v setcap >/dev/null; then
      $SU setcap cap_sys_nice=ep "$(readlink -f "$HOME/.local/lib/yami-niri/current/niri")" || true
    fi
    fork_tidy
  else
    FAILED+=("yami-niri")
  fi
elif ((DRY)); then
  printf '  %s·%s %s%s%s\n' "$A" "$R" "$T" "$(t "yami-niri: сборка и включение со следующего входа" "yami-niri: build and enable from the next login")" "$R"
fi

# ── summary ─────────────────────────────────────────────────────────────────
printf '\n  %s────────────────────────────────────────%s\n' "$F" "$R"
if ((${#FAILED[@]})); then
  warn "$(t "не вышло:" "failed:") ${FAILED[*]}"
  note "$(t "подробности:" "details:") $LOG"
else
  printf '  %s%s%s %s%s %s%s\n' "$OK" "$(t "готово." "done.")" "$R" "$D" "$(t "лог:" "log:")" "$LOG" "$R"
fi
if [[ -s $MISSING_F ]]; then
  warn "$(t "этих пакетов нет в репозиториях" "not in the repositories") ($OS_NAME): $(sort -u "$MISSING_F" | tr '\n' ' ')"
  note "$(t "шелл работает и без них; поставь аналоги вручную, если нужно" "the shell works without them; install equivalents by hand if needed")"
fi
if [[ -s $STATE/packages-needed ]] && ((!DRY)); then
  warn "$(t "для выбранного не хватает пакетов — одна команда (или настройки → обновления):" "the chosen components miss packages; one command (or settings → updates):")"
  note "$("$BIN/yami-shell" update need --print 2>/dev/null)"
fi
if [[ $FAM == nixos ]]; then
  printf '\n  %s%s%s\n' "$T" "$(t "добавь в environment.systemPackages:" "add to environment.systemPackages:")" "$R"
  printf '%s\n' "$NIX_PKGS" | sed "s/^/    $D/; s/\$/$R/"
  note "programs.niri.enable = true;"
fi
niri_ok || ((DRY)) || on fork || warn "$(t "нужен niri $NIRI_MIN или новее — поставь его или запусти без --no-fork" "niri $NIRI_MIN or newer is required: install it or rerun without --no-fork")"

if ((RUNNING_NIRI)); then next="$(t "всё уже применилось; Qt и GTK-программы — после перезапуска" "everything is live; Qt and GTK apps pick it up after a restart")"
else next="$(t "в экране входа выбери сессию «Niri» (или niri-session из TTY)" "pick the «Niri» session at login (or run niri-session from a TTY)")"; fi
k() { printf '  %s%s%s %s%s%s %s%s%s %s%s%s\n' "$A" "$(pad "$1" 16)" "$R" "$D" "$(pad "$2" 22)" "$R" "$A" "$(pad "$3" 16)" "$R" "$D" "$4" "$R"; }
printf '\n  %s%s%s%s  %s%s%s\n\n' "$T" "$B" "$(t "дальше" "next")" "$R" "$T" "$next" "$R"
k "Mod+Space" "$(t "программы" "launcher")" "Mod+V" "$(t "буфер обмена" "clipboard")"
k "Mod+S" "$(t "настройки, палитра" "settings, palette")" "Mod+Shift+Space" "$(t "стол / лента" "desk / tiling")"
k "Mod+Escape" "$(t "блокировка" "lock")" "Mod+N" "$(t "свернуть окно" "minimize")"
k "Mod+Shift+E" "$(t "питание" "power")" "Mod+Shift+/" "$(t "все клавиши" "all keys")"
k "Mod+Shift+S" "$(t "скриншот" "screenshot")" "Mod+E" "$(t "файлы" "files")"
printf '\n  %s%s%s\n' "$F" "$(t "клавиши меняются в настройках → клавиши; всё остальное: yami help, yami config list" "rebind keys in settings → keys; everything else: yami help, yami config list")" "$R"
printf '  %s%s%s\n' "$F" "$(t "обновления: настройки → обновления или yami update" "updates: settings → updates or yami update")" "$R"
if on fork; then
  printf '  %s%s ~/.local/lib/yami-niri/yami-niri.sh status | off%s\n\n' "$F" "$(t "yami-niri:" "yami-niri:")" "$R"
else
  printf '  %s%s %s/fork/yami-niri.sh build%s\n\n' "$F" "$(t "наш niri для игр и плавности:" "our niri for games and smoothness:")" "$ROOT" "$R"
fi

exit 0
}
