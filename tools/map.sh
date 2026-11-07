#!/usr/bin/env bash
# The repo in one screen: every source file with its header line and size. Cheaper than opening
# files to find where something lives. Usage: tools/map.sh [path-substring]
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
files=(shell/px/src/*.rs shell/src/*.rs shell/files/src/*.rs shell/view/src/*.rs shell/play/src/*.rs shell/greet/src/*.rs
  install.sh installer/*.sh tools/*.sh tools/*.py fork/*.sh system/greet/greet-session)
for f in "${files[@]}"; do
  [[ -n ${1:-} && $f != *"$1"* ]] && continue
  # the first comment or docstring line, without the comment marker
  h="$(grep -m1 -vE '^(#!|\s*$)' "$f" | sed -E 's@^\s*(//+|#+|"""|/\*+)\s*@@; s@"""\s*$@@')"
  printf '%-30s %5s  %s\n' "$f" "$(wc -l <"$f")" "$h"
done
