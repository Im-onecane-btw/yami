#!/usr/bin/env bash
# One command before every commit: build without warnings, i18n (static + every panel rendered
# in English), shell and Python syntax, niri config, installer dry run
# in a throwaway HOME.
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

step "i18n"
python3 -I "$ROOT/tools/check-i18n.py" || bad "static i18n"
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

step "login screen (SDDM theme, offscreen)"
QML="$(command -v qml6 || command -v qml || { [[ -x /usr/lib/qt6/bin/qml ]] && echo /usr/lib/qt6/bin/qml; } || true)"
if [[ -n $QML ]] && python3 -c 'import PIL' 2>/dev/null; then
  if err="$(timeout 20 "$ROOT/tools/sddm-preview.sh" "$TMP/sddm.png" 2>&1)" && [[ -s $TMP/sddm.png ]] && ! grep -qiE 'error|is not a type|is not defined|unavailable' <<<"$err"; then
    ok "Main.qml renders"
  else
    grep -iE 'error|warn|not a type|not defined' <<<"$err" | head -10
    bad "sddm theme"
  fi
else
  echo "qml or python-pillow missing, skipped"
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
