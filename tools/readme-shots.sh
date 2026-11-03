#!/usr/bin/env bash
# Real screenshots of a running yami session for the README (.github/assets): the desktop with the
# launcher, settings tabs, yami-files on a demo folder (闇 as in fastfetch on each palette's
# wallpaper), yami-view on one of them, the screenshot overlay (`yami-shell shot screenshot --frame`: the same drawing code
# with a selection and marks, over a real frame of the screen); in English and Russian. Runs on an empty workspace with do-not-disturb on (no live notification or
# window gets into a picture), the generated pixel wallpaper instead of your own picture, sound
# devices under plain names (audioNames) and drives hidden in yami-files, and puts the workspace,
# language, do-not-disturb, wallpaper and names back after.
# Refuses while a game or a fullscreen window has focus. Needs niri, grim, python3 with Pillow.
# Usage: tools/readme-shots.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/.github/assets"
TMP="$(mktemp -d)"
RU=0
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in ru*) RU=1 ;; esac
t() { if ((RU)); then printf '%s' "$1"; else printf '%s' "$2"; fi; }
say() { printf '\e[38;2;196;22;28m▪\e[0m %s\n' "$*"; }
die() { printf '\e[31m✕ %s\e[0m\n' "$*" >&2; exit 1; }

[[ -n ${NIRI_SOCKET:-} ]] || die "$(t "нужен сеанс niri" "needs a niri session")"
f="$(niri msg --json focused-window 2>/dev/null || true)"
if grep -qiE '"app_id":"(steam_app_[0-9]+|gamescope|[^"]*\.exe)"' <<<"$f" || grep -q '"is_fullscreen":true' <<<"$f"; then
  die "$(t "в фокусе игра или окно на весь экран — не снимаю" "a game or a fullscreen window has focus: not shooting")"
fi
ws() { niri msg --json workspaces | python3 -I -c "$1"; }
out="$(niri msg --json focused-output | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["name"])')"
back="$(ws 'import json,sys; print(next(w["idx"] for w in json.load(sys.stdin) if w["is_focused"]))')"
empty="$(ws "import json,sys; print(max(w['idx'] for w in json.load(sys.stdin) if w['output']=='$out' and w['active_window_id'] is None))")"
lang0="$(yami config get lang)"
dnd0="$(yami config get dnd)"
names0="$(yami config get audioNames)"
wall0="$(yami config get wallpaper)"
pids=()
# демо-папка с коротким путём для строки адреса; удаляется, только если её создали мы
demo="${TMPDIR:-/tmp}/yami"
[[ -e $demo ]] && die "$(t "$demo уже есть — убери его" "$demo exists: remove it first")"
made=0
restore() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  yami config set lang "$lang0" >/dev/null
  yami config set dnd "$dnd0" >/dev/null
  yami config set audioNames "$names0" >/dev/null
  yami config set wallpaper "$wall0" >/dev/null
  niri msg action focus-workspace "$back" >/dev/null 2>&1 || true
  rm -rf "$TMP"
  ((made)) && rm -rf "$demo"
  return 0
}
trap restore EXIT

# демо-папка: свои картинки проекта (обои всех семи палитр), ничего из домашней папки
made=1
mkdir -p "$demo"/{Documents,Downloads,Music,Projects,Pictures}
python3 -I - "$demo" "$ROOT" <<'PY'
import re, sys
from pathlib import Path
d, root = Path(sys.argv[1]), Path(sys.argv[2])
sys.path.insert(0, str(root / "tools"))
import theme  # noqa: E402
from PIL import Image  # noqa: E402
# 闇 как в fastfetch (theme.KANJI: полосы от светлого к акценту, тень в пиксель) на обоях палитры
k = theme.KANJI
for pid, hx in re.findall(r'Preset \{ id: "(\w+)", name: "[^"]*", accent: 0x([0-9a-f]{6})', (root / "shell/px/src/theme.rs").read_text()):
    p = theme.derive("#" + hx)
    im = theme.wallpaper_img(p)
    bands = [p["bright"], theme.mix(p["bright"], p["accent"], 0.5), p["accent"], theme.mix(p["accent"], p["deep"], 0.5)]
    shadow = theme.mix(p["deep"], p["bg"], 0.45)
    on = lambda x, y: 0 <= y < len(k) and 0 <= x < len(k[0]) and k[y][x] == "#"
    cell, (w, h) = 8, im.size
    x0, y0 = (w - len(k[0]) * cell) // 2, (h - len(k) * cell) // 2
    for y in range(len(k) + 1):
        for x in range(len(k[0]) + 1):
            c = bands[min(y * 4 // len(k), 3)] if on(x, y) else shadow if on(x - 1, y - 1) and not on(x, y) else None
            if c:
                im.paste(tuple(int(c.lstrip("#")[i:i + 2], 16) for i in (0, 2, 4)), (x0 + x * cell, y0 + y * cell, x0 + (x + 1) * cell, y0 + (y + 1) * cell))
    im.resize((w * 4, h * 4), Image.NEAREST).save(d / f"{pid}.png")
(d / "notes.md").write_text("# yami\n\nPixel-art desktop for niri.\n")
(d / "todo.txt").write_text("ship 2.1.0\n")
(d / "Projects/yami.tar.gz").write_bytes(b"\x1f\x8b" + bytes(4000))
(d / "Music/night.flac").write_bytes(bytes(9000))
PY

cfg="$TMP/cfg"
mkdir -p "$cfg"
for f in yami-shell.json yami-files.json user-dirs.dirs; do
  [[ -f ${XDG_CONFIG_HOME:-$HOME/.config}/$f ]] && cp "${XDG_CONFIG_HOME:-$HOME/.config}/$f" "$cfg/"
done
# диски и флешки — не в кадр
python3 -I - "$cfg/yami-files.json" <<'PY'
import json, sys
from pathlib import Path
p = Path(sys.argv[1])
try:
    d = json.loads(p.read_text())
except (OSError, ValueError):
    d = {}
d["devices"] = False
p.write_text(json.dumps(d))
PY
# звуковые устройства — простыми именами вместо моделей: микрофон и наушники
names() { # lang
  pactl -f json list sinks 2>/dev/null | python3 -I -c '
import json, sys
ru = sys.argv[1] == "ru"
mic, out = ("USB-микрофон", "Наушники") if ru else ("USB microphone", "Headphones")
names, n = {}, 0
for s in json.load(sys.stdin):
    t = (s.get("name", "") + " " + s.get("description", "")).lower()
    if "mic" in t:
        names[s["name"]] = mic
    else:
        n += 1
        names[s["name"]] = out if n == 1 else f"{out} {n}"
print(json.dumps(names, ensure_ascii=False))' "$1"
}

shoot() { sleep "${2:-1.2}"; grim -o "$out" "$TMP/$1.png"; }
# панель по центру экрана с полями (видно затемнённый стол вокруг)
crop() { # name w h
  python3 -I - "$TMP/$1.png" "$2" "$3" "$4" <<'PY'
import sys
from PIL import Image
src, w, h, dst = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
im = Image.open(src)
m = 28
x, y = (im.width - w) // 2 - m, (im.height - h) // 2 - m
im.crop((x, y, x + w + 2 * m, y + h + 2 * m)).save(dst, optimize=True)
PY
}
full() { python3 -I -c 'import sys; from PIL import Image; Image.open(sys.argv[1]).convert("RGB").save(sys.argv[2], optimize=True)' "$TMP/$1.png" "$2"; }

say "$(t "пустой рабочий стол $empty, не беспокоить, пиксельные обои" "empty workspace $empty, do not disturb, the pixel wallpaper")"
yami config set dnd true >/dev/null
yami config set wallpaper '""' >/dev/null
niri msg action focus-workspace "$empty"
for lang in en ru; do
  say "$(t "язык" "language"): $lang"
  yami config set lang "$lang" >/dev/null
  yami config set audioNames "$(names "$lang")" >/dev/null
  sleep 2
  yami msg launcher
  shoot "desktop"
  yami msg launcher
  full desktop "$OUT/desktop-$lang.png"
  for tab in color updates agents keys; do
    yami msg "settings $tab"
    shoot "settings-$tab"
    crop "settings-$tab" 780 572 "$OUT/settings-$tab-$lang.png"
  done
  yami msg settings
  yami msg "popup sound"
  shoot "sound" 1
  python3 -I -c 'import sys; from PIL import Image; im=Image.open(sys.argv[1]); im.crop((im.width-560, 0, im.width, 300)).save(sys.argv[2], optimize=True)' "$TMP/sound.png" "$OUT/popup-sound-$lang.png"
  yami msg "popup sound"
  # свой XDG_CONFIG_HOME: вид и папки XDG те же, а закладок GTK нет
  XDG_CONFIG_HOME="$cfg" yami-files "$demo" >/dev/null 2>&1 &
  pids+=($!)
  sleep 1.5
  niri msg action maximize-column
  shoot "files" 2
  full files "$OUT/files-$lang.png"
  # оверлей скриншота: выделение и метки (код снимка шелла) поверх настоящего кадра с файлами
  yami-shell shot screenshot --lang "$lang" --frame "$TMP/files.png" -o "$TMP/snap" >/dev/null
  full snap/screenshot "$OUT/screenshot-$lang.png"
  kill "${pids[@]}" 2>/dev/null || true
  pids=()
  # просмотрщик — на обоях «сакуры»
  yami-view "$demo/sakura.png" >/dev/null 2>&1 &
  pids+=($!)
  shoot "view" 2
  full view "$OUT/view-$lang.png"
  kill "${pids[@]}" 2>/dev/null || true
  pids=()
  sleep 1
done
say "$(t "готово:" "done:") $OUT"
