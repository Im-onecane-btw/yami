# Rust (the distro's if new enough, else rustup into ~/.cargo) and the build of
# the shell, yami-files, yami-view, yami-play and yami-greet into the cache, then the binaries into
# ~/.local/bin, each swapped in whole.

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
  # each binary as a copy next to it, then a rename: the running shell keeps its old file, and a
  # full disk or a cut copy leaves the previous binary in place, never half of a new one
  local b
  for b in yami-shell yami-files yami-view yami-play; do
    install -Dm755 "$CACHE/target/release/$b" "$BIN/.$b.new" && mv -f "$BIN/.$b.new" "$BIN/$b" || { rm -f "$BIN/.$b.new"; return 1; }
  done
  # `yami <command>`: the same binary, help instead of the shell when started without a command
  ln -sfn yami-shell "$BIN/yami"
}
