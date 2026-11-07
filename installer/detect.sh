# What this machine is. The distro family and its package manager, sudo or
# doas, memory and threads (how hard to build, whether yami-niri is worth it), the GPU (NVIDIA alone
# or next to another one), systemd, a running niri, the login manager in use and Steam. Defaults
# of the components come from here; the flags of install.sh override them.

detect_distro() {
  local id pm
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
  # NixOS: the system is declared (flake.nix: the NixOS and Home Manager modules), never changed here
  [[ -z $PM || $PM == nix ]] && SUDO=0
  return 0
}

detect_machine() {
  local j v
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
  return 0
}

# the login screen is on by default only when no other login manager is set; ours is greetd with
# yami-greet, or SDDM with the old yami theme (the login screen before 2.6: an update moves it over)
detect_login() {
  local d m
  OTHER_DM=""
  if [[ -L /etc/systemd/system/display-manager.service ]]; then
    OTHER_DM="$(basename "$(readlink /etc/systemd/system/display-manager.service)" .service)"
  else
    # OpenRC (Alpine, Gentoo) and runit (Void): a login manager in the default runlevel / enabled services
    for d in /etc/runlevels/default /var/service /etc/runit/runsvdir/default; do
      [[ -d $d ]] || continue
      for m in greetd sddm gdm lightdm lxdm ly emptty cosmic-greeter plasmalogin display-manager xdm slim; do
        [[ -e $d/$m ]] && { OTHER_DM=$m; break 2; }
      done
    done
  fi
  case $OTHER_DM in
    greetd) grep -qs 'yami/greet-session' /etc/greetd/config.toml && OTHER_DM="" ;;
    sddm) [[ -d /usr/share/sddm/themes/yami ]] && OTHER_DM="" ;;
  esac
  LOGIN_ON=1
  [[ -n $OTHER_DM ]] && LOGIN_ON=0
  [[ -n $LOGIN_FLAG ]] && LOGIN_ON=$LOGIN_FLAG
  # games: on by default only when Steam is here (a package, flatpak or its folder)
  GAMES_ON=0
  { command -v steam >/dev/null || [[ -d $HOME/.local/share/Steam || -d $HOME/.steam || -d $HOME/.var/app/com.valvesoftware.Steam ]]; } && GAMES_ON=1
  [[ -n $GAMES_FLAG ]] && GAMES_ON=$GAMES_FLAG
  return 0
}
