#!/usr/bin/env bash
# One command before every commit: build without warnings, i18n (static + every panel rendered
# in English), the English and Russian docs in step, shell and Python syntax, niri config,
# installer dry run in a throwaway HOME.
# Touches nothing outside the repo and the temp dir. Usage: tools/check.sh [--fast]
#   --fast  skip the installer dry run
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAST=0
[[ ${1:-} == --fast ]] && FAST=1
fail=0
step() { printf '\e[1m── %s\e[0m\n' "$1"; }
bad() { printf '\e[31m✗ %s\e[0m\n' "$1"; fail=1; }
ok() { printf '\e[32m✓ %s\e[0m\n' "$1"; }

step "build (release, warnings are errors)"
out="$(cargo build --release --locked --workspace --message-format short --manifest-path "$ROOT/shell/Cargo.toml" 2>&1)"
if [[ $? -ne 0 ]]; then
  printf '%s\n' "$out" | grep -E '^(error|.*: error)' | head -20
  bad "build failed"
elif grep -q 'warning:' <<<"$out"; then
  grep 'warning:' <<<"$out" | grep -v 'generated [0-9]* warning' | head -20
  bad "build has warnings"
else
  ok "build"
fi
BIN="$ROOT/shell/target/release/yami-shell"

step "versions"
want="$(cat "$ROOT/VERSION")"
vbad=0
for f in "$ROOT"/shell/Cargo.toml "$ROOT"/shell/*/Cargo.toml; do
  [[ -f $f ]] || continue
  v="$(sed -n '0,/^version = "\(.*\)"/s//\1/p' "$f")"
  [[ $v == "$want" ]] || { echo "${f#"$ROOT"/}: $v, VERSION: $want"; vbad=1; }
done
((vbad)) && bad "crate versions differ from VERSION (tools/release.sh sets them)" || ok "every crate is $want"

step "i18n"
python3 -I "$ROOT/tools/check-i18n.py" || bad "static i18n"
# README, CHANGELOG, docs/*, fork/README and their .ru.md twins: one skeleton, links, anchors
python3 -I "$ROOT/tools/check-docs.py" && ok "docs: English and Russian in step" || bad "docs EN/RU (tools/check-docs.py)"
if [[ -x $BIN ]]; then
  # every panel drawn offscreen in English: any Russian text left on screen fails
  "$BIN" shot --defaults --lang en -o "$TMP/en" | grep untranslated
  [[ ${PIPESTATUS[0]} -eq 0 ]] && ok "shot --lang en: no Russian left" || bad "shot --lang en"
  "$BIN" shot --defaults --lang ru -o "$TMP/ru" >/dev/null || bad "shot --lang ru (render crashed?)"
fi
FILES="$ROOT/shell/target/release/yami-files"
if [[ -x $FILES ]]; then
  "$FILES" shot --lang en -o "$TMP/fen" | grep untranslated
  [[ ${PIPESTATUS[0]} -eq 0 ]] && ok "yami-files shot --lang en: no Russian left" || bad "yami-files shot --lang en"
  "$FILES" shot --lang ru -o "$TMP/fru" >/dev/null || bad "yami-files shot --lang ru (render crashed?)"
fi

VIEW="$ROOT/shell/target/release/yami-view"
if [[ -x $VIEW ]]; then
  "$VIEW" shot --lang en -o "$TMP/ven" | grep untranslated
  [[ ${PIPESTATUS[0]} -eq 0 ]] && ok "yami-view shot --lang en: no Russian left" || bad "yami-view shot --lang en"
fi
PLAY="$ROOT/shell/target/release/yami-play"
if [[ -x $PLAY ]]; then
  "$PLAY" shot --lang en -o "$TMP/pen" | grep untranslated
  [[ ${PIPESTATUS[0]} -eq 0 ]] && ok "yami-play shot --lang en: no Russian left" || bad "yami-play shot --lang en"
fi
step "unit tests"
cargo test --release --locked --workspace --quiet --manifest-path "$ROOT/shell/Cargo.toml" >"$TMP/test.log" 2>&1 && ok "cargo test" || { tail -20 "$TMP/test.log"; bad "cargo test"; }

step "login screen (yami-greet: offscreen states, niri config)"
GREET="$ROOT/shell/target/release/yami-greet"
if [[ -x $GREET ]]; then
  "$GREET" shot --lang en -o "$TMP/gen" | grep untranslated
  [[ ${PIPESTATUS[0]} -eq 0 ]] && ok "yami-greet shot --lang en: no Russian left" || bad "yami-greet shot --lang en"
  NIRI_BIN="$(command -v niri || true)"
  if [[ -n $NIRI_BIN ]]; then
    for v in "" --plain; do
      "$GREET" kdl $v >"$TMP/greet.kdl" && "$NIRI_BIN" validate -c "$TMP/greet.kdl" >/dev/null 2>&1 && ok "niri takes yami-greet kdl $v" || bad "yami-greet kdl $v: niri rejects it"
    done
  fi
  sh -n "$ROOT/system/greet/greet-session" || bad "syntax: system/greet/greet-session"
fi

step "scripts"
for f in "$ROOT"/install.sh "$ROOT"/tools/*.sh "$ROOT"/fork/*.sh; do
  bash -n "$f" || bad "syntax: ${f#"$ROOT"/}"
done
if command -v shellcheck >/dev/null; then
  shellcheck -S error "$ROOT"/install.sh "$ROOT"/tools/*.sh "$ROOT"/fork/*.sh || bad "shellcheck"
fi
python3 -I -c 'import ast,sys; [ast.parse(open(f).read(), f) for f in sys.argv[1:]]' "$ROOT"/tools/*.py || bad "python syntax"
ok "scripts parsed"

# checks of a local checkout that are not part of the repository
if [[ -x $ROOT/tools/check.local.sh ]]; then
  step "local"
  "$ROOT/tools/check.local.sh" || bad "tools/check.local.sh"
fi

step "niri config"
if command -v niri >/dev/null; then
  niri validate -c "$ROOT/config/niri/config.kdl" >/dev/null 2>&1 && ok "config.kdl valid" || bad "niri validate config.kdl"
else
  echo "niri not installed, skipped"
fi

if ((!FAST)); then
  step "installer dry run (throwaway HOME)"
  mkdir -p "$TMP/home"
  if HOME="$TMP/home" NIRI_SOCKET='' XDG_CONFIG_HOME='' XDG_CACHE_HOME='' XDG_STATE_HOME='' XDG_DATA_HOME='' \
    timeout 60 "$ROOT/install.sh" --yes --dry-run --no-anim --no-sudo --lang en >"$TMP/dry.log" 2>&1; then
    ok "install.sh --dry-run"
  else
    tail -20 "$TMP/dry.log"
    bad "install.sh --dry-run"
  fi
fi

echo
if ((fail)); then
  printf '\e[31mcheck: FAILED\e[0m\n'
  exit 1
fi
printf '\e[32mcheck: all good\e[0m\n'
