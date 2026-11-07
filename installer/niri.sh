# Niri itself. Upstream niri from source when the distro's is older than 26.04,
# and yami-niri (fork/): built when its patch series moved, its realtime capability, and its sources
# (~4 GB) removed after a build unless they hold patch work.

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

# update: yami-niri only when the patch series moved (5–25 min), else the built one stays
fork_stale() {
  local want have
  want="$(sed -n 's/^VERSION="\(.*\)"/\1/p' "$ROOT/fork/yami-niri.sh")"
  have="$("$HOME/.local/lib/yami-niri/current/niri" --version 2>/dev/null | awk '{print $2}')"
  [[ $want != "$have" ]]
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

# yami-niri: build (5–25 min, the output stays on the terminal), realtime priority without the
# rtprio limit (the capability on the binary, dropped at start), then the sources go
fork_build() {
  need_rust && "$ROOT/fork/yami-niri.sh" build 2>&1 | tee -a "$LOG" || return 1
  if [[ "$(ulimit -r)" == 0 ]] && ((SUDO)) && command -v setcap >/dev/null; then
    $SU setcap cap_sys_nice=ep "$(readlink -f "$HOME/.local/lib/yami-niri/current/niri")" || true
  fi
  fork_tidy
}
