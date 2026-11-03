#!/usr/bin/env bash
# Preview the yami SDDM theme without SDDM: builds the theme into a temp dir with your current
# palette and wallpaper, then renders it offscreen to a PNG (or opens a window with --window).
#   tools/sddm-preview.sh [OUT.png] [--window] [--lang ru|en]
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
OUT="" WINDOW=0 LANG_SET=""
while (($#)); do
  case "$1" in
    --window) WINDOW=1 ;;
    --lang) LANG_SET="$2"; shift ;;
    *) OUT="$1" ;;
  esac
  shift
done
[[ -z $OUT && $WINDOW == 0 ]] && OUT="$PWD/sddm-preview.png"
QML="$(command -v qml6 || command -v qml || echo /usr/lib/qt6/bin/qml)"
THEME="$TMP/yami"
"$ROOT/tools/sddm-build.sh" "$THEME" >/dev/null
[[ -n $LANG_SET ]] && printf 'lang=%s\n' "$LANG_SET" >>"$THEME/theme.conf.user"
# theme.conf (+ .user) → JS object for the mock `config`
cfg="$(python3 -I - "$THEME" <<'PY'
import json, sys
from pathlib import Path
d = {}
for f in ("theme.conf", "theme.conf.user"):
    p = Path(sys.argv[1]) / f
    if p.exists():
        for l in p.read_text().splitlines():
            l = l.strip()
            if l and not l.startswith(("#", "[")) and "=" in l:
                k, v = l.split("=", 1)
                d[k.strip()] = v.strip()
print(json.dumps(d, ensure_ascii=False))
PY
)"
sed -e "s|THEME/Main.qml|$THEME/Main.qml|" -e "s|property var config: ({})|property var config: ($cfg)|" "$ROOT/sddm/preview/Preview.qml" >"$TMP/Preview.qml"
if ((WINDOW)); then
  "$QML" "$TMP/Preview.qml"
else
  QT_QPA_PLATFORM=offscreen "$QML" "$TMP/Preview.qml" -- "$OUT"
  echo "$OUT"
fi
