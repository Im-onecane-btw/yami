#!/usr/bin/env bash
# CI inside a distro container (.github/workflows/distros.yml): every package the installer
# would ask for must exist in this distro's repositories, the build dependencies install, the
# whole workspace builds from source with them, and install.sh --dry-run passes.
# A broken package list of one family shows up here, not in an issue.
#   tools/ci-distro.sh            check names, install the build deps, build, dry run
#   tools/ci-distro.sh --names    only check the package names
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ONLY_NAMES=0
[[ ${1:-} == --names ]] && ONLY_NAMES=1

say() { printf '\n── %s\n' "$*"; }

say "package lists"
LISTS="$(bash "$ROOT/install.sh" --print-packages)"
printf '%s\n' "$LISTS"
FAM="$(sed -n 's/^family: //p' <<<"$LISTS")"
pkgs() { sed -n "s/^$1: //p" <<<"$LISTS"; }

# refresh the package index once
case $FAM in
  arch) pacman -Sy --noconfirm >/dev/null ;;
  fedora) dnf -q makecache >/dev/null ;;
  debian) apt-get update -q >/dev/null ;;
  suse) zypper -q refresh >/dev/null ;;
  void) xbps-install -S >/dev/null ;;
  alpine) apk update -q >/dev/null ;;
  *) echo "unknown family: $FAM"; exit 1 ;;
esac

# does a package exist in the repositories (not: is it installed)
exists() {
  case $FAM in
    arch) pacman -Si "$1" >/dev/null 2>&1 || pacman -Sg "$1" >/dev/null 2>&1 ;;
    fedora) [[ -n "$(dnf -q repoquery --qf '%{name}' "$1" 2>/dev/null)" ]] ;;
    debian) apt-cache show "$1" >/dev/null 2>&1 ;;
    suse) zypper -q info "$1" 2>/dev/null | grep -q '^Name' ;;
    void) xbps-query -R "$1" >/dev/null 2>&1 ;;
    alpine) [[ -n "$(apk search -x "$1" 2>/dev/null)" ]] ;;
  esac
}

say "do the names exist"
fail=0
for list in P_build P_base P_apps P_look P_login P_nirisrc P_niri; do
  missing=()
  for p in $(pkgs "$list"); do
    exists "$p" || missing+=("$p")
  done
  if ((${#missing[@]} == 0)); then
    echo "ok   $list"
  elif [[ $list == P_build ]]; then
    # without these nothing builds: an error
    echo "FAIL $list: ${missing[*]}"
    fail=1
  else
    # optional on some releases (niri itself is not packaged everywhere): a warning
    echo "warn $list: ${missing[*]}"
  fi
done
((fail)) && exit 1
((ONLY_NAMES)) && exit 0

say "build dependencies"
read -ra build <<<"$(pkgs P_build)"
case $FAM in
  arch) pacman -S --noconfirm --needed "${build[@]}" git curl >/dev/null ;;
  fedora) dnf -q -y install "${build[@]}" git curl >/dev/null ;;
  debian) DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends "${build[@]}" git curl ca-certificates >/dev/null ;;
  suse) zypper -q -n install "${build[@]}" git curl >/dev/null ;;
  void) xbps-install -y "${build[@]}" git curl bash >/dev/null ;;
  alpine) apk add -q "${build[@]}" git curl bash >/dev/null ;;
esac

# the same rule as the installer: the distro's rust if new enough, otherwise rustup
RUST_MIN="$(sed -n 's/^RUST_MIN="\(.*\)"/\1/p' "$ROOT/install.sh")"
have="$(rustc --version 2>/dev/null | awk '{print $2}')"
if [[ -z $have || "$(printf '%s\n%s\n' "$RUST_MIN" "$have" | sort -V | head -n1)" != "$RUST_MIN" ]]; then
  echo "rustc ${have:-none} < $RUST_MIN: rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path >/dev/null
  export PATH="$HOME/.cargo/bin:$PATH"
fi
rustc --version

say "build (release, locked, warnings are errors)"
RUSTFLAGS="-D warnings" cargo build --release --locked --workspace --manifest-path "$ROOT/shell/Cargo.toml"

say "installer dry run"
# the installer runs as a user: a throwaway one with its own home
id yamici >/dev/null 2>&1 || useradd -m yamici 2>/dev/null || adduser -D yamici
su yamici -s /bin/bash -c "cd '$ROOT' && ./install.sh --yes --dry-run --no-anim --no-sudo --no-count --lang en" >/tmp/dry.log 2>&1 || { tail -40 /tmp/dry.log; exit 1; }
tail -5 /tmp/dry.log
echo "distro check: all good ($FAM)"
