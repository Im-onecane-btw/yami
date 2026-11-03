#!/usr/bin/env bash
# Assemble the yami SDDM theme into DIR: QML, compiled shader, bundled pixel fonts, the current
# wallpaper and a theme.conf.user with the shell palette (via tools/theme.py --sddm).
#   tools/sddm-build.sh DIR
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="${1:?usage: sddm-build.sh DIR}"
mkdir -p "$DIR/fonts"
cp "$ROOT"/sddm/yami/*.qml "$ROOT"/sddm/yami/metadata.desktop "$ROOT"/sddm/yami/theme.conf "$ROOT"/sddm/yami/dissolve.frag.qsb "$DIR/"
cp "$ROOT/fonts/Tiny5.ttf" "$ROOT/fonts/DotGothic16-Regular.ttf" "$ROOT"/fonts/OFL-*.txt "$DIR/fonts/"
python3 -I "$ROOT/tools/theme.py" --sddm "$DIR"
echo "$DIR"
