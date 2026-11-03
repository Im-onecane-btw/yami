"""Pictures for the README in .github/assets: the logo, a whole desktop, feature cards and panel
shots, in both languages. Everything comes from the project itself: panels are drawn by
`yami-shell shot --defaults` (made-up windows, apps and notifications), colours by theme.py
`derive`, the wallpaper by theme.py `wallpaper`, icons from gfx.rs `icon7`, fonts from fonts/.
Needs Pillow (as theme.py) and a built shell.  Usage: tools/readme-art.py [path/to/yami-shell]"""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import theme  # noqa: E402

OUT = ROOT / ".github/assets"
SHELL = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "shell/target/release/yami-shell"
P = theme.derive("#c4161c")
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


def pxbox(d, x, y, w, h, fill, edge, shadow=True):
    """прямоугольник со срезанными углами, рамка 1 px и жёсткая тень (как gfx.rs pxbox)"""
    def notched(x, y, w, h, col):
        d.rectangle([x + 2, y, x + w - 3, y + h - 1], fill=col)
        d.rectangle([x, y + 2, x + w - 1, y + h - 3], fill=col)
        d.point([(x + 1, y + 1), (x + w - 2, y + 1), (x + 1, y + h - 2), (x + w - 2, y + h - 2)], fill=col)
    if shadow:
        notched(x + 3, y + 3, w, h, (0, 0, 0, 160))
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
    ("ram", "~15 MB shell", "bar, launcher, dock, notifications and widgets:", "one Rust binary, no toolkit, own pixels",
     "шелл на ~15 МБ", "бар, лаунчер, док, уведомления и виджеты:", "один бинарь на Rust, без тулкитов"),
    ("drop", "one accent", "pick a colour: Qt, KDE, GTK, kitty, browsers,", "cursor and the login screen follow",
     "один акцент", "выбери цвет: Qt, KDE, GTK, kitty, браузеры,", "курсор и экран входа перекрасятся"),
    ("play", "made for games", "yami-niri: tearing, VRR in games; while you play", "notifications wait, the profile is «performance»",
     "для игр", "yami-niri: tearing, VRR в играх; пока играешь,", "уведомления ждут, профиль «производительность»"),
    ("folder", "files and viewer", "yami-files with the portal file dialog for every", "app, yami-view for pictures",
     "файлы и картинки", "yami-files и диалог выбора файлов для всех", "программ, yami-view для картинок"),
    ("down", "signed updates", "settings or yami update: built from source,", "tags checked by the author's key, rollback",
     "подписанные обновления", "из настроек или yami update: сборка из исходников,", "проверка ключом автора, откат"),
    ("keys", "any distro", "Arch, Fedora, Debian, openSUSE, Void, Alpine,", "Gentoo, NixOS: one installer, from source",
     "любой дистрибутив", "Arch, Fedora, Debian, openSUSE, Void, Alpine,", "Gentoo, NixOS: один установщик, из исходников"),
]


def card(c, lang):
    ic, *rest = c
    title, l1, l2 = rest[:3] if lang == "en" else rest[3:]
    W, H = 424, 92
    im = Image.new("RGBA", (W + 4, H + 4), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    pxbox(d, 0, 0, W, H, rgb("panel"), rgb("border"))
    dissolve(d, W - 120, 1, 118, 4, rgb("deep"))
    pxbox(d, 12, 14, 31, 31, rgb("accent"), rgb("bright"), shadow=False)
    icon(d, ic, 17, 19, 3, rgb("onAccent"))
    text(d, (56, 12), title.upper(), 16, rgb("text"))
    text(d, (56, 44), l1, 8, rgb("textDim"))
    text(d, (56, 60), l2, 8, rgb("textDim"))
    d.rectangle([56, 34, 56 + 22, 35], fill=rgb("accent"))
    return im.resize((im.width * 2, im.height * 2), Image.NEAREST)


def logo():
    """闇 из DotGothic16 (16 px, без сглаживания) на плашке акцента — SVG из квадратов"""
    g = Image.new("1", (20, 20), 0)
    d = ImageDraw.Draw(g)
    d.fontmode = "1"
    d.text((2, 1), "闇", font=ImageFont.truetype(DOT, 16), fill=1)
    px = [(x, y) for y in range(20) for x in range(20) if g.getpixel((x, y))]
    ac, br, bg = P["accent"], P["bright"], P["bg"]
    tile = f'<path fill="{ac}" d="M2 0h16v1h1v1h1v16h-1v1h-1v1H2v-1H1v-1H0V2h1V1h1z"/>'
    edge = f'<path fill="{br}" d="M2 0h16v1H2zM0 2h1v1H0z"/>'
    dots = "".join(f'<rect x="{x}" y="{y}" width="1" height="1"/>' for x, y in px)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" shape-rendering="crispEdges">{tile}{edge}'
            f'<g fill="{bg}">{dots}</g></svg>\n')


def shots(lang, tmp):
    targets = ["bar", "launcher", "dock", "notif", "widget:clock", "widget:graph", "settings:color", "settings:updates-new",
               "settings:agents", "settings:keys", "popup:sound", "screenshot"]
    out = Path(tmp) / lang
    for flag, sub in (("--transparent", "t"), ("", "o")):
        args = [str(SHELL), "shot", "--defaults", "--lang", lang, "-o", str(out / sub), *([flag] if flag else []), *targets]
        subprocess.run(args, check=True, stdout=subprocess.DEVNULL)
    # для стола — прозрачные (на обои), для галереи — как в документации, на подложке
    t = {f.stem: Image.open(f).convert("RGBA") for f in (out / "t").glob("*.png")}
    o = {f.stem: Image.open(f).convert("RGBA") for f in (out / "o").glob("*.png")}
    return t, o


def desktop(s, tmp):
    """весь стол: пиксельные обои палитры ×4, бар, виджеты, карточка, лаунчер, док"""
    theme.HOME = Path(tmp)
    theme.wallpaper(P)
    W, H = 1920, 1080
    im = Image.open(Path(tmp) / ".local/state/yami/wallpaper.png").convert("RGBA").resize((W, H), Image.NEAREST)
    d = ImageDraw.Draw(im)
    # иероглиф обоев, как рисует шелл (wallKanji): крупно, тоном рамки
    k = Image.new("1", (20, 20), 0)
    kd = ImageDraw.Draw(k)
    kd.fontmode = "1"
    kd.text((2, 1), "闇", font=ImageFont.truetype(DOT, 16), fill=1)
    for y in range(20):
        for x in range(20):
            if k.getpixel((x, y)):
                d.rectangle([W - 420 + x * 18, H - 470 + y * 18, W - 420 + x * 18 + 17, H - 470 + y * 18 + 17], fill=rgb("border", 110))
    im.alpha_composite(s["bar"], (0, 0))
    im.alpha_composite(s["widget-clock"], (48, 72))
    im.alpha_composite(s["widget-graph"], (48, 72 + s["widget-clock"].height + 16))
    im.alpha_composite(s["notif"], (W - s["notif"].width - 12, 44))
    im.alpha_composite(s["launcher"], ((W - s["launcher"].width) // 2, (H - s["launcher"].height) // 2 - 40))
    im.alpha_composite(s["dock"], ((W - s["dock"].width) // 2, H - s["dock"].height))
    return im


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "logo.svg").write_text(logo())
    with tempfile.TemporaryDirectory() as tmp:
        for lang in ("en", "ru"):
            s, solid = shots(lang, tmp)
            desktop(s, tmp).convert("RGB").save(OUT / f"desktop-{lang}.png", optimize=True)
            for c in CARDS:
                card(c, lang).save(OUT / f"card-{c[0]}-{lang}.png", optimize=True)
            for k in ("settings-color", "settings-updates-new", "settings-agents", "settings-keys", "screenshot", "popup-sound"):
                solid[k].save(OUT / f"{k.replace('-new', '')}-{lang}.png", optimize=True)
    print(f"readme-art: {len(list(OUT.iterdir()))} files in {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
