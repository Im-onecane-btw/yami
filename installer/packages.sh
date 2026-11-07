# The package names of every distro family, the package manager (a refresh,
# an install that drops names a release lacks and goes on) and what the chosen components still miss
# (packages-needed: an update has no sudo, the shell offers one command for it). NixOS has no lists:
# flake.nix with its NixOS and Home Manager modules is the way there.
# shellcheck shell=bash

# niri — the compositor; base — what the shell and the session use at runtime;
# build — to compile the shell; apps — kitty, fastfetch, pavucontrol, btop; drives — udisks2 and gvfs
# (drives, phones, network shares); files — yami-files' thumbnails and archives; play — libmpv for
# yami-play; look — the Qt/GTK theme; games — GameMode, MangoHud;
# tools — night light, text and QR from screenshots; login — greetd for yami-greet (agreety: the text
# fallback); nirisrc — to build niri itself (and setcap for yami-niri's realtime priority)
packages_for_family() {
case "$FAM" in
  arch)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulse wireplumber libpulse networkmanager bluez bluez-utils
      cliphist wl-clipboard grim brightnessctl playerctl python python-pillow noto-fonts noto-fonts-cjk noto-fonts-emoji
      power-profiles-daemon libxkbcommon pam fontconfig curl)
    P_build=(rust gcc pkgconf libxkbcommon pam)
    P_apps=(kitty fastfetch pavucontrol btop)
    P_drives=(udisks2 gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb)
    P_files=(ffmpegthumbnailer libarchive)
    P_play=(mpv)
    P_look=(qt6ct adw-gtk-theme papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract tesseract-data-eng tesseract-data-rus zbar)
    P_login=(greetd greetd-agreety)
    P_nirisrc=(git clang pkgconf libinput libxkbcommon mesa libdisplay-info seatd pango cairo pipewire systemd-libs libcap) ;;
  fedora)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulseaudio wireplumber pulseaudio-utils NetworkManager bluez
      cliphist wl-clipboard grim brightnessctl playerctl python3 python3-pillow google-noto-sans-fonts google-noto-sans-cjk-fonts
      google-noto-color-emoji-fonts tuned-ppd libxkbcommon pam fontconfig curl)
    P_build=(rust cargo gcc pkgconf-pkg-config libxkbcommon-devel pam-devel)
    P_apps=(kitty fastfetch pavucontrol btop)
    P_drives=(udisks2 gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb gvfs-fuse)
    P_files=(ffmpegthumbnailer bsdtar)
    P_play=(mpv-libs)
    P_look=(qt6ct adw-gtk3-theme papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract tesseract-langpack-eng tesseract-langpack-rus zbar)
    P_login=(greetd)
    P_nirisrc=(git clang libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-devel systemd-devel
      libseat-devel pipewire-devel pango-devel cairo-gobject-devel libdisplay-info-devel libcap) ;;
  debian)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulse wireplumber pulseaudio-utils network-manager bluez
      cliphist wl-clipboard grim brightnessctl playerctl python3 python3-pil fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji
      power-profiles-daemon libxkbcommon0 libpam0g fontconfig curl)
    P_build=(cargo rustc gcc pkg-config libxkbcommon-dev libpam0g-dev)
    P_apps=(kitty fastfetch pavucontrol btop)
    P_drives=(udisks2 gvfs gvfs-backends gvfs-fuse)
    P_files=(ffmpegthumbnailer libarchive-tools)
    P_play=(libmpv2)
    P_look=(qt6ct adw-gtk3 papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-eng tesseract-ocr-rus zbar-tools)
    P_login=(greetd)
    P_nirisrc=(git clang libudev-dev libgbm-dev libxkbcommon-dev libegl1-mesa-dev libwayland-dev libinput-dev libdbus-1-dev
      libsystemd-dev libseat-dev libpipewire-0.3-dev libpango1.0-dev libdisplay-info-dev libcap2-bin) ;;
  suse)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulseaudio wireplumber pulseaudio-utils NetworkManager bluez
      cliphist wl-clipboard grim brightnessctl playerctl python3 python3-Pillow google-noto-sans-fonts google-noto-sans-cjk-fonts
      noto-coloremoji-fonts power-profiles-daemon libxkbcommon0 pam fontconfig curl)
    P_build=(rust cargo gcc pkgconf libxkbcommon-devel pam-devel)
    P_apps=(kitty fastfetch pavucontrol btop)
    P_drives=(udisks2 gvfs gvfs-backends gvfs-backend-afc gvfs-fuse)
    P_files=(ffmpegthumbnailer bsdtar)
    P_play=(libmpv2)
    P_look=(qt6ct gtk3-metatheme-adwaita papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-traineddata-eng tesseract-ocr-traineddata-rus zbar)
    P_login=(greetd)
    P_nirisrc=(git clang libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-1-devel systemd-devel
      libseat-devel pipewire-devel pango-devel cairo-devel libdisplay-info-devel libcap-progs) ;;
  void)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire wireplumber pulseaudio-utils NetworkManager bluez
      cliphist wl-clipboard grim brightnessctl playerctl python3 python3-Pillow noto-fonts-ttf noto-fonts-cjk noto-fonts-emoji
      power-profiles-daemon libxkbcommon pam fontconfig curl)
    P_build=(rust cargo gcc pkg-config libxkbcommon-devel pam-devel)
    P_apps=(kitty fastfetch pavucontrol btop)
    P_drives=(udisks2 gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb)
    P_files=(ffmpegthumbnailer bsdtar)
    P_play=(mpv)
    P_look=(qt6ct adw-gtk3 papirus-icon-theme)
    P_games=(gamemode MangoHud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-eng tesseract-ocr-rus zbar)
    P_login=(greetd)
    P_nirisrc=(git clang eudev-libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-devel
      libseat-devel pipewire-devel pango-devel libdisplay-info-devel libcap-progs) ;;
  alpine)
    P_niri=(niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk)
    P_base=(polkit gnome-keyring pipewire pipewire-pulse wireplumber pulseaudio-utils networkmanager networkmanager-cli bluez
      cliphist wl-clipboard grim brightnessctl playerctl python3 py3-pillow font-noto font-noto-cjk font-noto-emoji
      power-profiles-daemon libxkbcommon linux-pam fontconfig curl)
    P_build=(rust cargo gcc musl-dev pkgconf libxkbcommon-dev linux-pam-dev)
    P_apps=(kitty fastfetch pavucontrol btop)
    P_drives=(udisks2 gvfs gvfs-mtp gvfs-gphoto2 gvfs-afc gvfs-smb gvfs-fuse)
    P_files=(ffmpegthumbnailer libarchive-tools)
    P_play=(mpv-libs)
    P_look=(qt6ct adw-gtk3 papirus-icon-theme)
    P_games=(gamemode mangohud)
    P_tools=(wlsunset tesseract-ocr tesseract-ocr-data-rus zbar)
    P_login=(greetd greetd-agreety)
    P_nirisrc=(git clang eudev-dev mesa-dev libxkbcommon-dev wayland-dev libinput-dev dbus-dev libseat-dev pipewire-dev
      pango-dev libdisplay-info-dev libcap-utils) ;;
  gentoo)
    P_niri=(gui-wm/niri gui-apps/xwayland-satellite sys-apps/xdg-desktop-portal-gnome sys-apps/xdg-desktop-portal-gtk)
    P_base=(sys-auth/polkit gnome-base/gnome-keyring media-video/pipewire media-video/wireplumber
      net-misc/networkmanager net-wireless/bluez gui-apps/cliphist gui-apps/wl-clipboard gui-apps/grim
      app-misc/brightnessctl media-sound/playerctl dev-python/pillow media-fonts/noto media-fonts/noto-cjk media-fonts/noto-emoji
      sys-power/power-profiles-daemon x11-libs/libxkbcommon sys-libs/pam net-misc/curl)
    P_build=(virtual/rust virtual/pkgconfig)
    P_apps=(x11-terms/kitty app-misc/fastfetch media-sound/pavucontrol sys-process/btop)
    P_drives=(sys-fs/udisks gnome-base/gvfs)
    P_files=(media-video/ffmpegthumbnailer app-arch/libarchive)
    P_play=(media-video/mpv)
    P_look=(gui-apps/qt6ct x11-themes/adw-gtk3 x11-themes/papirus-icon-theme)
    P_games=(games-util/gamemode games-util/mangohud)
    P_tools=(gui-apps/wlsunset app-text/tesseract media-gfx/zbar)
    P_login=(gui-libs/greetd)
    P_nirisrc=(dev-vcs/git llvm-core/clang dev-libs/libinput sys-auth/seatd media-libs/mesa x11-libs/pango media-libs/libdisplay-info sys-libs/libcap) ;;
  *)
    P_niri=() P_base=() P_build=() P_apps=() P_drives=() P_files=() P_play=() P_look=() P_games=() P_tools=() P_login=() P_nirisrc=() ;;
esac
}
# name: packages, one list per line: tools/ci-distro.sh checks that each exists in the repositories
print_packages() {
  local a
  printf 'family: %s\n' "$FAM"
  for a in P_niri P_base P_build P_apps P_drives P_files P_play P_look P_games P_tools P_login P_nirisrc; do
    declare -n arr=$a
    printf '%s: %s\n' "$a" "${arr[*]}"
  done
}

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
  wants drives && want+=("${P_drives[@]}")
  wants files && want+=("${P_files[@]}")
  wants play && want+=("${P_play[@]}")
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
