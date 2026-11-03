"""Drawn pictures for the README in .github/assets: the pixel logo and the feature cards (one picture per
language), from the project itself: colours by theme.py `derive`, icons from gfx.rs `icon7`, fonts
from fonts/. Screenshots of a real session come from tools/readme-shots.sh.
Needs Pillow (as theme.py).  Usage: tools/readme-art.py [#accent]  (default: the shell's palette)"""
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import theme  # noqa: E402

OUT = ROOT / ".github/assets"
def accent():
    """акцент — тот же, что на скриншотах (палитра шелла), а без настроек — «кровь»"""
    try:
        import json
        st = json.loads((Path.home() / ".config/yami-shell.json").read_text())
    except (OSError, ValueError):
        st = {}
    presets = dict(re.findall(r'Preset \{ id: "(\w+)", name: "[^"]*", accent: 0x([0-9a-f]{6})', (ROOT / "shell/px/src/theme.rs").read_text()))
    if re.fullmatch(r"#?[0-9a-fA-F]{6}", st.get("accent") or ""):
        return "#" + st["accent"].lstrip("#")
    return "#" + presets.get(st.get("palette", "blood"), "c4161c")


P = theme.derive(sys.argv[1] if len(sys.argv) > 1 else accent())
TINY = str(ROOT / "fonts/Tiny5.ttf")
DOT = str(ROOT / "fonts/DotGothic16-Regular.ttf")


def rgb(k, a=255):
    h = P[k].lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), a)


def icons():
    src = (ROOT / "shell/px/src/gfx.rs").read_text()
    return {m[0]: re.findall(r'"([.#]{7})"', m[1]) for m in re.findall(r'"(\w+)" => &\[((?:"[.#]{7}",? ?){7})\]', src)}


ICONS = icons()
BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5]


def text(d, xy, s, size, col):
    d.fontmode = "1"  # целые пиксели, без сглаживания
    d.text(xy, s, font=ImageFont.truetype(TINY, size), fill=col)


def pxbox(d, x, y, w, h, fill, edge, shadow=(0, 0, 0, 160)):
    """прямоугольник со срезанными углами, рамка 1 px и жёсткая тень (как gfx.rs pxbox); на непрозрачном
    фоне тень — сплошной цвет: ImageDraw не смешивает альфу, а заменяет пиксель"""
    def notched(x, y, w, h, col):
        d.rectangle([x + 2, y, x + w - 3, y + h - 1], fill=col)
        d.rectangle([x, y + 2, x + w - 1, y + h - 3], fill=col)
        d.point([(x + 1, y + 1), (x + w - 2, y + 1), (x + 1, y + h - 2), (x + w - 2, y + h - 2)], fill=col)
    if shadow:
        notched(x + 3, y + 3, w, h, shadow)
    notched(x, y, w, h, edge)
    notched(x + 1, y + 1, w - 2, h - 2, fill)


def icon(d, name, x, y, cell, col):
    for j, row in enumerate(ICONS.get(name, [])):
        for i, c in enumerate(row):
            if c == "#":
                d.rectangle([x + i * cell, y + j * cell, x + (i + 1) * cell - 1, y + (j + 1) * cell - 1], fill=col)


def dissolve(d, x, y, w, h, col):
    """полоса Байера: густо слева, на нет вправо"""
    for yy in range(h):
        for xx in range(w):
            if BAYER[(yy & 3) * 4 + (xx & 3)] / 16 < 1 - xx / w:
                d.point((x + xx, y + yy), fill=col)


CARDS = [
    ("ram", "~15 MB shell", "bar, launcher, dock, notifications, widgets:", "one Rust binary, no toolkit, no GPU context",
     "шелл на 15 МБ", "бар, лаунчер, док, уведомления, виджеты:", "один бинарь на Rust, без тулкитов и GPU"),
    ("drop", "one accent", "pick a colour: Qt, KDE, GTK, kitty, browser,", "the cursor and the login screen follow",
     "один акцент", "выбери цвет: Qt, KDE, GTK, kitty, браузер,", "курсор и экран входа перекрасятся"),
    ("play", "made for games", "tearing and VRR on yami-niri; while you play", "notifications wait, power goes to max",
     "для игр", "tearing и VRR на yami-niri; пока играешь,", "уведомления ждут, питание на максимум"),
    ("folder", "files and viewer", "yami-files: tabs, split view, trash and the", "file dialog of every app; yami-view",
     "файлы и картинки", "yami-files: вкладки, корзина и диалог", "выбора файлов для всех; yami-view"),
    ("camera", "screenshots", "area, window or screen; loupe, colour picker,", "pen, arrows, mosaic, numbered badges",
     "скриншоты", "область, окно или экран; лупа, пипетка,", "перо, стрелки, мозаика, номерки"),
    ("eye", "stream curtain", "viewers of a stream see a frozen, blurred", "screen while you keep working",
     "шторка стрима", "зрители видят размытый стоп-кадр,", "а ты спокойно работаешь дальше"),
    ("down", "signed updates", "one click or yami update, built from source,", "tags checked by the author's key, rollback",
     "обновления", "в один клик или yami update, сборка", "из исходников, подпись автора, откат"),
    ("screen", "any distro", "Arch, Fedora, Debian, openSUSE, Void, Alpine,", "Gentoo, NixOS: one installer, one command",
     "8 дистрибутивов", "Arch, Fedora, Debian, openSUSE, Void, Alpine,", "Gentoo, NixOS: один установщик"),
]
HEAD = {"en": ("yami", "a pixel desktop for niri"), "ru": ("yami", "пиксельный рабочий стол для niri")}


def width(s, size):
    return ImageFont.truetype(TINY, size).getlength(s)


def features(lang):
    """все карточки в одном окне шелла: шапка, точечный фон, сетка 2×N — одна картинка, без таблицы GitHub"""
    W, pad, gap, hh, ch = 452, 10, 8, 18, 60
    cw = (W - 2 * pad - gap) // 2
    rows = (len(CARDS) + 1) // 2
    H = hh + pad + rows * ch + (rows - 1) * gap + pad
    im = Image.new("RGBA", (W + 4, H + 4), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    pxbox(d, 0, 0, W, H, rgb("bg"), rgb("border"))
    for yy in range(hh + 6, H - 2, 8):  # точечная сетка, как на обоях
        for xx in range(6, W - 2, 8):
            d.point((xx, yy), fill=rgb("surface"))
    d.rectangle([2, 1, W - 3, hh - 1], fill=rgb("panel"))
    d.rectangle([1, 2, W - 2, hh - 1], fill=rgb("panel"))
    d.line([1, hh, W - 2, hh], fill=rgb("border"))
    d.rectangle([10, hh // 2 - 3, 15, hh // 2 + 2], fill=rgb("accent"))
    title, sub = HEAD[lang]
    text(d, (22, 4), title, 8, rgb("text"))
    text(d, (24 + width(title, 8), 4), sub, 8, rgb("textFaint"))
    dissolve(d, W - 130, 2, 120, hh - 3, rgb("surface"))
    for k, c in enumerate(CARDS):
        ic, *rest = c
        t, l1, l2 = rest[:3] if lang == "en" else rest[3:]
        x, y = pad + (k % 2) * (cw + gap), hh + pad + (k // 2) * (ch + gap)
        tw = cw - 46 - 8
        for s, size in ((t.upper(), 16), (l1, 8), (l2, 8)):
            if width(s, size) > tw:
                sys.exit(f"readme-art: «{s}» is {width(s, size):.0f} px wide, the card has {tw}")
        pxbox(d, x, y, cw, ch, rgb("panel"), rgb("border"), shadow=rgb("codeBg"))
        pxbox(d, x + 10, y + 10, 25, 25, rgb("accent"), rgb("bright"), shadow=None)
        icon(d, ic, x + 12, y + 12, 3, rgb("onAccent"))
        text(d, (x + 46, y + 6), t.upper(), 16, rgb("text"))
        d.rectangle([x + 46, y + 27, x + 46 + 15, y + 28], fill=rgb("accent"))
        text(d, (x + 46, y + 33), l1, 8, rgb("textDim"))
        text(d, (x + 46, y + 44), l2, 8, rgb("textDim"))
    return im.resize((im.width * 2, im.height * 2), Image.NEAREST)


def logo():
    """闇 как в fastfetch (theme.KANJI, полосы от светлого к акценту и тень в пиксель) на тёмной
    плашке со срезанными углами — SVG из квадратов, читается и в светлой теме GitHub"""
    k = theme.KANJI
    gw, gh = len(k[0]), len(k)
    pad = 3
    W, H = gw + 1 + 2 * pad, gh + 2 * pad
    bands = [P["bright"], theme.mix(P["bright"], P["accent"], 0.5), P["accent"], theme.mix(P["accent"], P["deep"], 0.5)]
    shadow = theme.mix(P["deep"], P["bg"], 0.45)
    on = lambda x, y: 0 <= y < gh and 0 <= x < gw and k[y][x] == "#"
    cells = {}
    for y in range(gh):
        for x in range(gw + 1):
            if on(x, y):
                cells.setdefault(bands[min(y * len(bands) // gh, len(bands) - 1)], []).append((x, y))
            elif on(x - 1, y - 1) and (x >= gw or y >= gh - 1):
                cells.setdefault(shadow, []).append((x, y))
    tile = (f'<path fill="{P["border"]}" d="M2 0h{W - 4}v1h1v1h1v{H - 4}h-1v1h-1v1H2v-1H1v-1H0V2h1V1h1z"/>'
            f'<path fill="{P["bg"]}" d="M2 1h{W - 4}v1h1v{H - 4}h-1v1H2v-1H1V2h1z"/>')
    glyph = "".join(f'<path fill="{c}" d="' + "".join(f"M{x + pad} {y + pad}h1v1h-1z" for x, y in pts) + '"/>' for c, pts in cells.items())
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" shape-rendering="crispEdges">{tile}{glyph}</svg>\n'


SOCIAL = ("yami", "a pixel-art desktop for niri", "Rust Wayland shell in ~15 MB · own file manager · one accent for every app")


def social():
    """превью ссылки (GitHub → Settings → Social preview, 1280×640): обои палитры, 闇 как в
    fastfetch и надписи Tiny5 — всё на сетке 2×2"""
    W, H, cell = 640, 320, 6
    im = theme.wallpaper_img(P, W, H).convert("RGBA")
    d = ImageDraw.Draw(im)
    k = theme.KANJI
    bands = [P["bright"], theme.mix(P["bright"], P["accent"], 0.5), P["accent"], theme.mix(P["accent"], P["deep"], 0.5)]
    shadow = theme.mix(P["deep"], P["bg"], 0.45)
    on = lambda x, y: 0 <= y < len(k) and 0 <= x < len(k[0]) and k[y][x] == "#"
    x0, y0 = 70, (H - len(k) * cell) // 2
    for y in range(len(k) + 1):
        for x in range(len(k[0]) + 1):
            c = bands[min(y * 4 // len(k), 3)] if on(x, y) else shadow if on(x - 1, y - 1) else None
            if c:
                d.rectangle([x0 + x * cell, y0 + y * cell, x0 + (x + 1) * cell - 1, y0 + (y + 1) * cell - 1], fill=rgb_of(c))
    tx = x0 + (len(k[0]) + 1) * cell + 40
    name, tag, line = SOCIAL
    text(d, (tx, 104), name, 48, rgb("text"))
    d.rectangle([tx, 160, tx + 31, 163], fill=rgb("accent"))
    text(d, (tx, 176), tag, 16, rgb("textDim"))
    text(d, (tx, 204), line, 8, rgb("textFaint"))
    return im.resize((W * 2, H * 2), Image.NEAREST).convert("RGB")


def rgb_of(h):
    h = h.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), 255)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "logo.svg").write_text(logo())
    for lang in ("en", "ru"):
        features(lang).save(OUT / f"features-{lang}.png", optimize=True)
    social().save(OUT / "social.png", optimize=True)
    print(f"readme-art: logo, features-{{en,ru}}.png ({len(CARDS)} cards) and social.png in {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
