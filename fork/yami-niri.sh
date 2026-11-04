#!/usr/bin/env bash
# yami-niri: niri for yami. Upstream niri (a pinned release) plus the patches in patches/, built
# for this CPU (fat LTO, one codegen unit, target-cpu=native). The packaged niri stays installed
# as a safety net: the session starts through fork/session.sh, which falls back to it when the
# fork fails to start. `niri msg` resolves to the fork through ~/.local/bin/niri.
#
#   fork/yami-niri.sh build    fetch, patch and build (5–25 min), enable from the next login
#   fork/yami-niri.sh on       enable an existing build
#   fork/yami-niri.sh off      back to the packaged niri (from the next login)
#   fork/yami-niri.sh status   which niri the next login starts
#   fork/yami-niri.sh clean    drop the sources and build cache (~4 GB), keep the binary
#
# Session does not start at all: Ctrl+Alt+F3, log in, `~/.local/lib/yami-niri/yami-niri.sh off`.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
NIRI_REPO="https://github.com/YaLTeR/niri.git"
NIRI_REV="8ed0da44d974c32c6877d2f4630c314da0717ecb" # v26.04
SMITHAY_REPO="https://github.com/Smithay/smithay.git"
SMITHAY_REV="ff5fa7df392cecfba049ffed55cdaa4e98a8e7ef" # what niri v26.04 pins
# .N is the series level (number of patches): the shell writes only config the running niri knows
VERSION="26.04-yami.5"
SRC="${YAMI_NIRI_SRC:-${XDG_CACHE_HOME:-$HOME/.cache}/yami/niri-src}"
LIB="$HOME/.local/lib/yami-niri"
UNITS="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/niri.service.d"
DROPIN="$UNITS/yami-niri.conf"
LINK="$HOME/.local/bin/niri"

case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in ru*) RU=1 ;; *) RU=0 ;; esac
t() { if ((RU)); then printf '%s' "$1"; else printf '%s' "$2"; fi; }
say() { printf '\e[38;2;196;22;28m▸\e[0m %s\n' "$*"; }
die() { printf '\e[38;2;255;59;47m✕ %s\e[0m\n' "$*" >&2; exit 1; }

checkout() { # dir repo rev
  [[ -d "$1/.git" ]] || git clone --quiet --filter=blob:none "$2" "$1"
  git -C "$1" cat-file -e "$3^{commit}" 2>/dev/null || git -C "$1" fetch --quiet origin
  git -C "$1" am --abort >/dev/null 2>&1 || true
  git -C "$1" checkout --quiet --force -B yami "$3"
  git -C "$1" clean -qfd
}

patch_tree() { # dir patches-dir
  local p
  for p in "$2"/*.patch; do
    git -C "$1" -c user.name=yami -c user.email=yami@localhost am --quiet --committer-date-is-author-date "$p" ||
      die "$(t "патч не лёг" "patch does not apply"): ${p#"$HERE"/}"
  done
}

build() {
  command -v cargo >/dev/null || die "$(t "нет cargo: поставь rust (или rustup)" "cargo not found: install rust (or rustup)")"
  command -v clang >/dev/null || die "$(t "нет clang: поставь clang" "clang not found: install clang")"
  mkdir -p "$SRC"
  say "$(t "исходники" "sources"): niri ${NIRI_REV:0:8} (v26.04), smithay ${SMITHAY_REV:0:8}"
  checkout "$SRC/yami-smithay" "$SMITHAY_REPO" "$SMITHAY_REV"
  checkout "$SRC/yami-niri" "$NIRI_REPO" "$NIRI_REV"
  patch_tree "$SRC/yami-smithay" "$HERE/patches/smithay"
  patch_tree "$SRC/yami-niri" "$HERE/patches/niri"
  say "$(t "сборка под" "building for") $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs) — $(t "это надолго" "this takes a while")"
  (
    cd "$SRC/yami-niri"
    export RUSTFLAGS="-C target-cpu=native" NIRI_BUILD_VERSION_STRING="$VERSION ($(git rev-parse --short=8 HEAD))"
    command -v mold >/dev/null && RUSTFLAGS+=" -C link-arg=-fuse-ld=mold"
    nice cargo build --profile yami --locked
  )
  local rev dir old
  rev="$(git -C "$SRC/yami-niri" rev-parse --short=8 HEAD)"
  dir="$LIB/$rev"
  mkdir -p "$dir"
  install -m 0755 "$SRC/yami-niri/target/yami/niri" "$dir/niri.new"
  mv -f "$dir/niri.new" "$dir/niri"
  local cfg="${XDG_CONFIG_HOME:-$HOME/.config}/niri/config.kdl"
  if [[ -f $cfg ]]; then
    "$dir/niri" validate -c "$cfg" >/dev/null 2>&1 ||
      die "$(t "конфиг не прошёл проверку новым niri — не включаю" "the config fails validation with the new niri, not enabling it")"
  fi
  ln -sfn "$dir" "$LIB/current"
  for old in "$LIB"/*/; do
    old="${old%/}"
    [[ $old == "$dir" || -L $old ]] || rm -rf -- "$old"
  done
  on
}

on() {
  [[ -x "$LIB/current/niri" ]] || die "$(t "yami-niri не собран" "yami-niri is not built"): fork/yami-niri.sh build"
  mkdir -p "$UNITS" "${LINK%/*}"
  install -m 0755 "$HERE/session.sh" "$LIB/session"
  [[ "$HERE" == "$LIB" ]] || install -m 0755 "$HERE/yami-niri.sh" "$LIB/yami-niri.sh"
  rm -f -- "$UNITS/dark-niri.conf" # the old fork's drop-in
  # NotifyAccess=all: READY=1 comes from niri, a child of the wrapper
  printf '%s\n' "# yami: the session runs yami-niri, falling back to the packaged niri (off: yami-niri.sh off)" \
    "[Service]" "NotifyAccess=all" "ExecStart=" "ExecStart=$LIB/session" >"$DROPIN"
  ln -sfn "$LIB/current/niri" "$LINK"
  systemctl --user daemon-reload 2>/dev/null || true
  say "$(t "yami-niri включится со следующего входа" "yami-niri is enabled from the next login") ($("$LINK" --version))"
  if [[ "$(ulimit -r)" == 0 ]] && ! getcap "$LIB/current/niri" 2>/dev/null | grep -q cap_sys_nice; then
    say "$(t "для realtime-приоритета (плавность под нагрузкой):" "for realtime priority (smooth under load):") sudo setcap cap_sys_nice=ep $(readlink -f "$LIB/current/niri")"
  fi
}

off() {
  rm -f -- "$DROPIN" "$UNITS/dark-niri.conf"
  # rules only the fork knows would break the packaged niri's config
  local fx="${XDG_CONFIG_HOME:-$HOME/.config}/niri/yami-fx.kdl"
  [[ -f $fx ]] && sed -i '/allow-tearing/d' "$fx"
  [[ "$(readlink "$LINK" 2>/dev/null)" == "$LIB"/* ]] && rm -f -- "$LINK"
  systemctl --user daemon-reload 2>/dev/null || true
  say "$(t "со следующего входа — niri из пакета" "packaged niri from the next login")"
}

case "${1:-status}" in
  build) build ;;
  on) on ;;
  off) off ;;
  clean)
    # only the trees this script makes: a YAMI_NIRI_SRC pointing at another folder is never removed
    for e in "$SRC"/* "$SRC"/.[!.]*; do
      [[ -e $e ]] || continue
      case ${e##*/} in yami-niri | yami-smithay) ;; *) echo "$(t "не папка исходников yami-niri, не трогаю:" "not a yami-niri source folder, left alone:") $SRC" >&2; exit 1 ;; esac
    done
    rm -rf -- "$SRC" && say "$(t "исходники и кеш сборки удалены" "sources and build cache removed")" ;;
  status)
    if [[ -f $DROPIN ]]; then
      echo "yami-niri: $("$LIB/current/niri" --version 2>/dev/null || echo "$(t "сломан" "broken")") — $(readlink -f "$LIB/current/niri")"
    else
      echo "$(t "niri из пакета" "packaged niri"): $(command -v niri)"
    fi
    ;;
  *) sed -n '2,/^set -eu/p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//'; exit 2 ;;
esac
