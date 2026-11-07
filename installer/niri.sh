# Niri itself. Upstream niri from source when the distro's is older than 26.04 (the one root step
# here), and yami-niri (fork/): built into ~/.local/lib when its patch series moved, its sources (~4 GB)
# removed after a build unless they hold patch work. Realtime priority is never granted behind your
# back: `yami-niri.sh status` says how (a realtime group or setcap) when the system has no rtprio limit.
# shellcheck shell=bash

plan_niri() {
  on core && add "$(t "niri $NIRI_MIN+ из исходников, если пакет старее" "niri $NIRI_MIN+ from source if the package is older" "niri $NIRI_MIN+ compilado se o pacote for mais antigo")" do_niri_source
  return 0
}
# after the steps: yami-niri is built last (minutes of output, nothing waits on it)
fork_after() {
  on fork || return 0
  if ((DRY)); then
    printf '  %s·%s %s%s%s\n' "$A" "$R" "$T" "$(t "yami-niri: сборка и включение со следующего входа" "yami-niri: build and enable from the next login" "yami-niri: compilar e ativar a partir do próximo login")" "$R"
    return 0
  fi
  ((!BUILD_FAILED)) || return 0
  if ((UPDATE)); then
    # an update builds it only when the patch series moved and nothing else failed
    ((${#FAILED[@]} == 0)) && fork_stale || return 0
    say "$(t "yami-niri: новая серия патчей — сборка (5–25 минут)" "yami-niri: a new patch series, building (5–25 minutes)" "yami-niri: nova série de patches, compilando (5–25 minutos)")"
  else
    printf '\n  %s%s%s\n\n' "$T" "$(t "сборка yami-niri — 5–25 минут, вывод прямо здесь:" "building yami-niri takes 5–25 minutes, output below:" "compilar o yami-niri leva de 5 a 25 minutos; saída abaixo:")" "$R"
  fi
  fork_build || FAILED+=("yami-niri")
}

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
  rm -rf -- "$src" && note "$(t "исходники yami-niri удалены (${sz:-?}): бинарник стоит, следующая сборка скачает их заново (--keep-build — оставить)" "yami-niri sources removed (${sz:-?}): the binary stays, the next build fetches them again (--keep-build keeps them)" "código-fonte do yami-niri removido (${sz:-?}): o binário permanece; a próxima compilação baixa novamente (--keep-build preserva)")"
}

# yami-niri: build (5–25 min, the output stays on the terminal), then the sources go
fork_build() {
  need_rust && "$ROOT/fork/yami-niri.sh" build 2>&1 | tee -a "$LOG" || return 1
  fork_tidy
}
