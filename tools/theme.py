#!/usr/bin/env python3
"""YAMI: перекраска всего вокруг шелла по одной палитре.

    theme.py [palette.json]       палитра от шелла (~/.local/state/yami/palette.json)
    theme.py --accent '#c4161c' [--light]   без шелла: вывести оттенки из акцента

Пишет: qt6ct (цвета + qss), схему KDE и kdeglobals, gtk.css для GTK3/4, цвета kitty, тему btop, MangoHud,
меню Firefox/LibreWolf (userChrome.css)
(открытые kitty перечитывают конфиг по SIGUSR1), fastfetch, цвета рамок niri
(~/.config/niri/yami-colors.kdl, niri подхватывает сам), обои шелла
(~/.local/state/yami/wallpaper.png), тему SDDM (theme.conf.user и обои в
/usr/share/sddm/themes/yami, если установщик сделал их твоими).

    theme.py --sddm DIR           только тему SDDM в DIR (сборка, предпросмотр)
    theme.py --login              только экран входа (шелл сменил обои)
    theme.py --cursor             только курсор (после смены его в настройках шелла)
    theme.py --profiles           вывести профили Firefox/LibreWolf/Floorp/Zen
    theme.py --fonts              только шрифты: размер по сетке Tiny5 под экран (или fontSize)
"""
import colorsys
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

HOME = Path.home()
CFG = Path(os.environ.get("XDG_CONFIG_HOME") or HOME / ".config")


# ── палитра ──────────────────────────────────────────────────────────────────
def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def hexc(c):
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(v))) for v in c)


def mix(a, b, t):
    a, b = rgb(a), rgb(b)
    return hexc(tuple(x + (y - x) * t for x, y in zip(a, b)))


def scale_v(h, k):
    r, g, b = (v / 255 for v in rgb(h))
    hh, s, v = colorsys.rgb_to_hsv(r, g, b)
    v *= k
    if v > 1:                 # как Qt.lighter: что не влезло в яркость — убираем из насыщенности
        s = max(0.0, s - (v - 1))
        v = 1.0
    return hexc(tuple(x * 255 for x in colorsys.hsv_to_rgb(hh, s, v)))


def light(p):
    """Светлая палитра (шелл пишет "mode": "light")."""
    return p.get("mode") == "light"


def hi(p):
    """Самый контрастный текст: белый в тёмной теме, чёрный в светлой (выделенное, наведение)."""
    return p.get("textHi", "#000000" if light(p) else "#ffffff")


def derive(accent, tint=1.0, light_mode=False):
    """Те же оттенки, что выводит шелл (theme.rs), — для запуска без шелла."""
    if light_mode:
        return derive_light(accent, tint)
    bg = mix("#080707", accent, 0.035 * tint)
    panel = mix("#0e0c0c", accent, 0.055 * tint)
    light = colorsys.rgb_to_hls(*(v / 255 for v in rgb(accent)))[1]
    return {
        "bg": bg, "panel": panel,
        "surface": mix("#161313", accent, 0.08 * tint),
        "surfaceHi": mix("#1f1b1b", accent, 0.12 * tint),
        "codeBg": mix("#050404", accent, 0.02 * tint),
        "border": mix(panel, accent, 0.3), "dim": mix(panel, accent, 0.45),
        "accent": accent, "deep": scale_v(accent, 1 / 1.6), "glow": scale_v(accent, 1 / 1.15),
        "bright": scale_v(accent, 1.3), "selection": mix(bg, accent, 0.55),
        "text": mix("#ebe5e4", accent, 0.05), "textDim": mix("#8c8282", accent, 0.12),
        "textFaint": mix("#5c5252", accent, 0.1),
        "onAccent": "#0b0909" if light > 0.6 else "#ffffff", "textHi": "#ffffff",
        "ok": "#78c878", "warn": "#ffaa28", "mode": "dark",
    }


def derive_light(accent, tint=1.0):
    """Светлая тема как в theme.rs (Theme::light): акцент темнеет до читаемого на бумаге."""
    lig = lambda h: colorsys.rgb_to_hls(*(v / 255 for v in rgb(h)))[1]
    a = accent
    for _ in range(12):
        if lig(a) <= 0.46:
            break
        a = scale_v(a, 0.9)
    bg = mix("#f4f0ed", accent, 0.04 * tint)
    panel = mix("#ebe6e2", accent, 0.06 * tint)
    text = mix("#1c1717", a, 0.06)
    return {
        "bg": bg, "panel": panel,
        "surface": mix("#ddd6d1", accent, 0.08 * tint),
        "surfaceHi": mix("#d0c8c2", accent, 0.12 * tint),
        "codeBg": mix("#fbf9f7", accent, 0.02 * tint),
        "border": mix(panel, a, 0.38), "dim": mix(panel, a, 0.55),
        "accent": a, "deep": scale_v(a, 1 / 1.35), "glow": scale_v(a, 1 / 1.1),
        "bright": scale_v(a, 0.82), "selection": mix(bg, accent, 0.3),
        "text": text, "textDim": mix("#5e5555", a, 0.1), "textFaint": mix("#958c89", a, 0.08),
        "onAccent": "#0b0909" if lig(a) > 0.6 else "#ffffff", "textHi": "#000000",
        "ok": "#2e8b3a", "warn": "#b86a00", "mode": "light",
    }


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.read_text() == text:
        return
    tmp = path.with_name(path.name + ".tmp")
    try:
        tmp.write_text(text)
        tmp.replace(path)
    except PermissionError:
        # папка чужая (тема SDDM у root), а сам файл наш: пишем на месте
        path.write_text(text)


# ── Qt: qt6ct ────────────────────────────────────────────────────────────────
def qt6ct(p):
    a = lambda h: "#ff" + h.lstrip("#")
    # порядок ролей QPalette: WindowText, Button, Light, Midlight, Dark, Mid, Text, BrightText,
    # ButtonText, Base, Window, Shadow, Highlight, HighlightedText, Link, LinkVisited,
    # AlternateBase, NoRole, ToolTipBase, ToolTipText, PlaceholderText
    act = [p["text"], p["surface"], p["border"], p["surfaceHi"], p["codeBg"], p["panel"], p["text"],
           hi(p), p["text"], p["bg"], p["panel"], "#000000", p["selection"], hi(p),
           p["bright"], p["accent"], mix(p["bg"], p["panel"], 0.6), "#000000", p["panel"], p["text"],
           p["textFaint"]]
    dis = list(act)
    for i in (0, 6, 8, 13, 19):
        dis[i] = p["textFaint"]
    dis[12] = p["surfaceHi"]
    ina = list(act)
    ina[12] = p["dim"]
    j = lambda xs: ", ".join(a(x) for x in xs)
    write(CFG / "qt6ct/colors/yami.conf",
          f"[ColorScheme]\nactive_colors={j(act)}\ndisabled_colors={j(dis)}\ninactive_colors={j(ina)}\n")
    d = CFG / "qt6ct/qss/yami"
    qt_pixels(p, d)
    u = lambda n: f"url({d / n}.png)"
    sl = lambda n: f"border-image: {u(n)} 4 4 4 4 stretch stretch; border-width: 4px;"
    write(CFG / "qt6ct/qss/yami.qss", f"""/* YAMI: written by tools/theme.py. Пиксельный вид шелла поверх Fusion: фаски, рамки и
   флажки — картинки 9-slice из палитры (qss/yami/), всё остальное — палитра qt6ct. */
QToolTip {{ background: {p['panel']}; color: {p['text']}; border: 1px solid {p['accent']}; padding: 3px; }}
QMenu {{ background: {p['panel']}; border: 1px solid {p['border']}; padding: 2px; }}
QMenu::item {{ padding: 4px 18px 4px 10px; }}
QMenu::item:selected {{ background: {p['selection']}; color: {hi(p)}; }}
QMenu::separator {{ height: 1px; background: {p['border']}; margin: 3px 4px; }}
QScrollBar:vertical {{ background: {p['bg']}; width: 10px; margin: 0; }}
QScrollBar:horizontal {{ background: {p['bg']}; height: 10px; margin: 0; }}
QScrollBar::handle {{ background: {p['border']}; min-height: 24px; min-width: 24px; }}
QScrollBar::handle:hover {{ background: {p['accent']}; }}
QScrollBar::add-line, QScrollBar::sub-line {{ width: 0; height: 0; }}
QScrollBar::add-page, QScrollBar::sub-page {{ background: none; }}
QPushButton {{ {sl('btn')} padding: 1px 10px; color: {p['text']}; min-height: 16px; }}
QPushButton:hover {{ {sl('btn-hover')} color: {hi(p)}; }}
QPushButton:pressed, QPushButton:checked {{ {sl('btn-press')} padding: 2px 10px 0 10px; }}
QPushButton:default {{ {sl('btn-accent')} color: {p['onAccent']}; }}
QPushButton:default:hover {{ {sl('btn-accent-hover')} }}
QPushButton:disabled {{ {sl('btn-off')} color: {p['textFaint']}; }}
QLineEdit, QAbstractSpinBox {{ {sl('field')} padding: 0 4px; color: {p['text']}; selection-background-color: {p['selection']}; }}
QLineEdit:focus, QAbstractSpinBox:focus {{ {sl('field-focus')} }}
QLineEdit:disabled, QAbstractSpinBox:disabled {{ color: {p['textFaint']}; }}
QTextEdit:focus, QPlainTextEdit:focus {{ border: 1px solid {p['accent']}; }}
QComboBox {{ {sl('btn')} padding: 0 6px; color: {p['text']}; min-height: 16px; }}
QComboBox:hover, QComboBox:on {{ {sl('btn-hover')} }}
QComboBox:editable {{ {sl('field')} }}
QComboBox::drop-down {{ border: 0; width: 16px; }}
QComboBox::down-arrow {{ image: {u('arrow')}; width: 7px; height: 7px; }}
QComboBox QAbstractItemView {{ background: {p['panel']}; border: 1px solid {p['border']}; selection-background-color: {p['selection']}; selection-color: {hi(p)}; outline: 0; }}
QCheckBox::indicator, QRadioButton::indicator, QGroupBox::indicator {{ width: 16px; height: 16px; }}
QCheckBox::indicator:unchecked, QGroupBox::indicator:unchecked {{ image: {u('check')}; }}
QCheckBox::indicator:unchecked:hover {{ image: {u('check-hover')}; }}
QCheckBox::indicator:checked, QGroupBox::indicator:checked {{ image: {u('check-on')}; }}
QCheckBox::indicator:checked:hover {{ image: {u('check-on-hover')}; }}
QCheckBox::indicator:indeterminate {{ image: {u('check-part')}; }}
QRadioButton::indicator:unchecked {{ image: {u('radio')}; }}
QRadioButton::indicator:unchecked:hover {{ image: {u('radio-hover')}; }}
QRadioButton::indicator:checked {{ image: {u('radio-on')}; }}
QProgressBar {{ {sl('field')} text-align: center; color: {p['text']}; }}
QProgressBar::chunk {{ background: {p['accent']}; }}
QSlider::groove:horizontal {{ height: 6px; background: {p['codeBg']}; border: 1px solid {p['border']}; }}
QSlider::sub-page:horizontal {{ background: {p['deep']}; border: 1px solid {p['border']}; }}
QSlider::handle:horizontal {{ background: {p['accent']}; width: 8px; margin: -6px 0; }}
QSlider::handle:horizontal:hover {{ background: {p['bright']}; }}
QTabBar::tab {{ background: {p['panel']}; color: {p['textDim']}; border: 1px solid {p['border']}; padding: 4px 12px; margin-right: 2px; }}
QTabBar::tab:hover {{ color: {p['text']}; }}
QTabBar::tab:selected {{ background: {p['surface']}; color: {p['text']}; border-bottom: 2px solid {p['accent']}; }}
QTabWidget::pane {{ border: 1px solid {p['border']}; }}
QHeaderView::section {{ background: {p['panel']}; color: {p['textDim']}; border: 0; border-right: 1px solid {p['border']}; border-bottom: 1px solid {p['border']}; padding: 3px 6px; }}
QGroupBox {{ border: 1px solid {p['border']}; margin-top: 14px; padding-top: 4px; }}
QGroupBox::title {{ subcontrol-origin: margin; left: 8px; padding: 0 4px; color: {p['bright']}; }}
""")


def qt_pixels(p, d):
    """Картинки для QSS: рамки со срезанными углами (как pxbox шелла: срез 2 px, рамка 1 px,
    фаска светом сверху-слева и тенью снизу-справа), флажки как в настройках шелла, стрелка."""
    from PIL import Image
    d.mkdir(parents=True, exist_ok=True)
    lift = lambda h, k: mix(h, "#ffffff", k)
    sink = lambda h, k: mix(h, "#000000", k)

    def box(name, fill, line, bevel=True, pressed=False, n=12):
        img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
        for y in range(n):
            for x in range(n):
                cx, cy = min(x, n - 1 - x), min(y, n - 1 - y)
                if cx + cy < 2:
                    continue  # срезанный угол
                c = line if (cx == 0 or cy == 0 or cx + cy == 2) else fill
                if c == fill and bevel and y == 1:
                    c = sink(fill, 0.25) if pressed else lift(fill, 0.12)  # свет сверху; нажатая — в тени
                elif c == fill and bevel and y == n - 2 and not pressed:
                    c = sink(fill, 0.35)
                img.putpixel((x, y), rgb(c) + (255,))
        img.save(d / f"{name}.png")

    box("btn", p["surface"], p["border"])
    box("btn-hover", p["surfaceHi"], p["accent"])
    box("btn-press", p["deep"], p["accent"], pressed=True)
    box("btn-accent", p["accent"], p["bright"])
    box("btn-accent-hover", p["bright"], hi(p))
    box("btn-off", p["panel"], p["border"], bevel=False)
    box("field", p["codeBg"], p["border"], bevel=False)
    box("field-focus", p["codeBg"], p["accent"], bevel=False)

    def mark(name, line, dot, shape):
        img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
        for y in range(16):
            for x in range(16):
                cx, cy = min(x, 15 - x), min(y, 15 - y)
                if cx + cy < 2:
                    continue
                c = line if (cx == 0 or cy == 0 or cx + cy == 2) else p["codeBg"]
                if dot and 4 <= x < 12 and 4 <= y < 12:
                    if shape == "dash" and not 7 <= y < 9:
                        pass
                    elif shape == "dot" and min(x - 4, 11 - x) + min(y - 4, 11 - y) < 2:
                        pass
                    else:
                        c = dot
                img.putpixel((x, y), rgb(c) + (255,))
        img.save(d / f"{name}.png")

    mark("check", p["border"], None, "")
    mark("check-hover", p["accent"], None, "")
    mark("check-on", p["border"], p["bright"], "box")
    mark("check-on-hover", p["accent"], p["bright"], "box")
    mark("check-part", p["border"], p["bright"], "dash")
    mark("radio", p["border"], None, "")
    mark("radio-hover", p["accent"], None, "")
    mark("radio-on", p["border"], p["bright"], "dot")
    arrow = Image.new("RGBA", (7, 7), (0, 0, 0, 0))
    for y, row in enumerate([".......", ".......", "#######", ".#####.", "..###..", "...#...", "......."]):
        for x, ch in enumerate(row):
            if ch == "#":
                arrow.putpixel((x, y), rgb(p["text"]) + (255,))
    arrow.save(d / "arrow.png")
    # диалоги файлов Qt — через портал, то есть yami-files (как у GTK и браузеров)
    _ini_set(CFG / "qt6ct/qt6ct.conf", "standard_dialogs", "xdgdesktopportal", "[Appearance]")


# ── KDE: схема цветов и kdeglobals ───────────────────────────────────────────
def kde(p):
    c = lambda h: ",".join(map(str, rgb(h)))
    groups = {
        "Window": (p["panel"], p["surface"]), "View": (p["bg"], mix(p["bg"], p["panel"], 0.6)),
        "Button": (p["surface"], p["surfaceHi"]), "Selection": (p["selection"], p["dim"]),
        "Tooltip": (p["panel"], p["surface"]), "Complementary": (p["codeBg"], p["panel"]),
        "Header": (p["panel"], p["surface"]),
    }
    out = ["[General]", "ColorScheme=Yami", "Name=YAMI", "shadeSortColumn=true", "",
           "[KDE]", "contrast=4", ""]
    for g, (bg, alt) in groups.items():
        fg = hi(p) if g == "Selection" else p["text"]
        out += [f"[Colors:{g}]", f"BackgroundNormal={c(bg)}", f"BackgroundAlternate={c(alt)}",
                f"ForegroundNormal={c(fg)}", f"ForegroundInactive={c(p['textDim'])}",
                f"ForegroundActive={c(p['bright'])}", f"ForegroundLink={c(p['bright'])}",
                f"ForegroundVisited={c(p['accent'])}", f"ForegroundNegative={c('#ff4a3d')}",
                f"ForegroundNeutral={c(p['warn'])}", f"ForegroundPositive={c(p['ok'])}",
                f"DecorationFocus={c(p['accent'])}", f"DecorationHover={c(p['bright'])}", ""]
    out += ["[Colors:Header][Inactive]", f"BackgroundNormal={c(p['bg'])}",
            f"BackgroundAlternate={c(p['panel'])}", f"ForegroundNormal={c(p['textDim'])}", "",
            "[WM]", f"activeBackground={c(p['panel'])}", f"activeForeground={c(p['text'])}",
            f"activeBlend={c(p['accent'])}", f"inactiveBackground={c(p['bg'])}",
            f"inactiveForeground={c(p['textFaint'])}", f"inactiveBlend={c(p['border'])}", ""]
    scheme = "\n".join(out)
    write(HOME / ".local/share/color-schemes/Yami.colors", scheme)

    def parse(text):
        res, cur = {}, None
        for line in text.splitlines():
            m = re.match(r"^(\[.*\])\s*$", line)
            if m:
                cur = m.group(1)
                res.setdefault(cur, {})
            elif cur and "=" in line and not line.lstrip().startswith("#"):
                k, v = line.split("=", 1)
                res[cur][k] = v
        return res

    target = CFG / "kdeglobals"
    cfg = parse(target.read_text()) if target.exists() else {}
    for sec, kv in parse(scheme).items():
        if sec.startswith("[Colors:") or sec == "[WM]":
            cfg[sec] = dict(kv)
        elif sec == "[General]":
            cfg.setdefault(sec, {})["ColorScheme"] = "Yami"
    # программы KDE сами включают Breeze, если он стоит; Fusion — чтобы работал наш QSS
    cfg.setdefault("[KDE]", {})["widgetStyle"] = "Fusion"
    write(target, "\n\n".join(s + "\n" + "\n".join(f"{k}={v}" for k, v in kv.items())
                              for s, kv in cfg.items()) + "\n")


# ── GTK 3/4 ──────────────────────────────────────────────────────────────────
def gtk(p):
    d = {
        "accent_color": p["bright"], "accent_bg_color": p["accent"], "accent_fg_color": p["onAccent"],
        "destructive_color": "#ff4a3d", "destructive_bg_color": "#8a1a14", "destructive_fg_color": "#ffffff",
        "success_color": p["ok"], "warning_color": p["warn"], "error_color": "#ff4a3d",
        "window_bg_color": p["panel"], "window_fg_color": p["text"],
        "view_bg_color": p["bg"], "view_fg_color": p["text"],
        "headerbar_bg_color": p["panel"], "headerbar_fg_color": p["text"],
        "headerbar_border_color": p["border"], "headerbar_backdrop_color": p["bg"],
        "sidebar_bg_color": mix(p["bg"], p["panel"], 0.5), "sidebar_fg_color": p["text"],
        "sidebar_backdrop_color": p["bg"],
        "card_bg_color": p["surface"], "card_fg_color": p["text"],
        "dialog_bg_color": p["panel"], "dialog_fg_color": p["text"],
        "popover_bg_color": p["panel"], "popover_fg_color": p["text"],
        "thumbnail_bg_color": p["surface"], "thumbnail_fg_color": p["text"],
        "borders": p["border"],
    }
    css = "/* YAMI: пишет tools/theme.py по палитре шелла. */\n"
    css += "".join(f"@define-color {k} {v};\n" for k, v in d.items())
    # жёсткие тени: в светлой теме — слабее, иначе панели обведены сажей
    sh = 0.22 if light(p) else 0.6
    css += f"""@define-color headerbar_shade_color rgba(0, 0, 0, {sh});
@define-color card_shade_color rgba(0, 0, 0, {sh});
@define-color shade_color rgba(0, 0, 0, {sh});
@define-color scrollbar_outline_color {"#ffffff" if light(p) else "#000000"};
""" + """
* { border-radius: 0; }
"""
    css += f"selection, *:selected {{ background-color: {p['selection']}; color: {hi(p)}; }}\n"
    css += f"popover > contents, menu, .menu, tooltip {{ border: 1px solid {p['border']}; }}\n"
    # пиксельный вид: без плавных переходов, кнопки с фаской и жёсткой тенью, как в шелле
    css += "* { transition: none; -gtk-icon-shadow: none; }\n"
    k = 0.4 if light(p) else 1.0
    css += (f"button {{ border: 1px solid {p['border']}; box-shadow: inset 1px 1px 0 alpha(white, {0.5 if light(p) else 0.06}), 2px 2px 0 alpha(black, {0.55 * k:.2f}); }}\n"
            f"button:hover {{ border-color: {p['accent']}; }}\n"
            f"button:active, button:checked {{ box-shadow: inset 2px 2px 0 alpha(black, {0.45 * k:.2f}); }}\n"
            f"button.suggested-action {{ background: {p['accent']}; color: {p['onAccent']}; }}\n"
            f"entry, spinbutton, dropdown > button, combobox > box > button {{ background: {p['codeBg']}; border: 1px solid {p['border']}; }}\n"
            f"entry:focus-within {{ border-color: {p['accent']}; outline: none; }}\n"
            f"window.csd, window.dialog, messagedialog, dialog-host > dialog {{ border: 1px solid {mix(p['border'], p['accent'], 0.5)}; box-shadow: 4px 4px 0 alpha(black, {0.6 * k:.2f}); }}\n")
    for v in ("3.0", "4.0"):
        write(CFG / f"gtk-{v}/gtk.css", css)
    gtk_mode(light(p))
    # акцент libadwaita/GNOME — ближайший из их набора по оттенку
    r, g, b = (x / 255 for x in rgb(p["accent"]))
    h, l, sat = colorsys.rgb_to_hls(r, g, b)
    if sat < 0.18:
        name = "slate"
    else:
        hues = [(0, "red"), (30, "orange"), (52, "yellow"), (120, "green"), (175, "teal"), (215, "blue"), (275, "purple"), (330, "pink"), (360, "red")]
        name = min(hues, key=lambda x: abs(x[0] - h * 360))[1]
    if shutil.which("gsettings"):
        subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "accent-color", name], check=False, capture_output=True)


def gtk_mode(is_light):
    """Светлая или тёмная основа GTK: тема adw-gtk3(-dark) и prefer-dark в settings.ini,
    color-scheme в gsettings (его читают GTK4/libadwaita и портал). Остальные ключи не трогаем."""
    theme = "adw-gtk3" if is_light else "adw-gtk3-dark"
    for v in ("3.0", "4.0"):
        ini = CFG / f"gtk-{v}/settings.ini"
        if not ini.exists():
            continue
        lines = ini.read_text().splitlines()
        want = {"gtk-theme-name": theme, "gtk-application-prefer-dark-theme": "false" if is_light else "true"}
        icons = next((ln.split("=", 1)[1].strip() for ln in lines if ln.startswith("gtk-icon-theme-name=")), "")
        other = _icon_pair(icons, is_light)
        if other:
            want["gtk-icon-theme-name"] = other
        out = []
        for ln in lines:
            k = ln.split("=", 1)[0].strip()
            if k in want:
                ln = f"{k}={want.pop(k)}"
            out.append(ln)
        out += [f"{k}={v2}" for k, v2 in want.items()]
        write(ini, "\n".join(out) + "\n")
    if shutil.which("gsettings"):
        for k, v in (("gtk-theme", theme), ("color-scheme", "prefer-light" if is_light else "prefer-dark")):
            subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", k, v], check=False, capture_output=True)
        cur = subprocess.run(["gsettings", "get", "org.gnome.desktop.interface", "icon-theme"], capture_output=True, text=True).stdout.strip().strip("'")
        other = _icon_pair(cur, is_light)
        if other:
            subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "icon-theme", other], check=False, capture_output=True)


# тёмные и светлые варианты значков: Papirus-Dark ↔ Papirus, Gruvbox-Plus-Dark ↔ Gruvbox-Plus-Light
ICON_PAIRS = (("Papirus-Dark", "Papirus"), ("Gruvbox-Plus-Dark", "Gruvbox-Plus-Light"))


def _icon_pair(name, is_light):
    """Вариант значков под тему, если он установлен; иначе None (тему значков не трогаем)."""
    for dark, lite in ICON_PAIRS:
        want, have = (lite, dark) if is_light else (dark, lite)
        if name == have:
            for d in (HOME / ".local/share/icons", HOME / ".icons", Path("/usr/share/icons")):
                if (d / want).is_dir():
                    return want
    return None


# ── шрифты: пиксельный UI-шрифт по сетке, размер под экран ───────────────────
UI_FONT = "Tiny5"
# Tiny5 нарисован на сетке 8 px на em: чётко только 16, 24, 32…
PIXEL_PX = (16, 24, 32)


_NAME_BAD = re.compile(r"[\x00-\x1f\x7f\"\\'`$;,{}<>|&#=]")


def _settings():
    """Настройки шелла. Строки отсюда попадают в kitty.conf, settings.ini, qt6ct и KDL: имена
    чистятся так же, как в шелле (config.rs sanitize), — перевод строки в имени шрифта из чужой
    темы иначе дописал бы в kitty.conf свою команду. Числа — в своих пределах."""
    try:
        st = json.loads((CFG / "yami-shell.json").read_text())
    except (OSError, ValueError):
        return {}
    if not isinstance(st, dict):
        return {}
    for k in ("fontUi", "fontMono", "fontShell", "fontFiles", "fontView", "cursor", "iconTheme"):
        v = st.get(k)
        st[k] = _NAME_BAD.sub("", v)[:64] if isinstance(v, str) else ""
    lang = st.get("lang")
    st["lang"] = lang if isinstance(lang, str) and lang.isascii() and lang.isalpha() else ""
    for k, lo, hi_, dflt in (("cursorSize", 16, 96, 24), ("fontSize", 0, 64, 0), ("fontPt", 0, 48, 0), ("wallPixel", 1, 64, 1)):
        try:
            st[k] = max(lo, min(hi_, int(st.get(k, dflt))))
        except (TypeError, ValueError):
            st[k] = dflt
    return st


def _screen_height():
    """Логическая высота самого большого экрана (масштаб niri уже учтён)."""
    try:
        r = subprocess.run(["niri", "msg", "-j", "outputs"], capture_output=True, text=True, timeout=3)
        hs = [o["logical"]["height"] for o in json.loads(r.stdout).values() if o.get("logical")]
        return max(hs) if hs else 1080
    except Exception:
        return 1080


def font_px():
    """Размер UI-шрифта: из настроек шелла (fontSize) или по экрану: до 1600 строк — 16,
    до 2200 (4K без масштаба) — 24, больше — 32."""
    v = int(_settings().get("fontSize") or 0)
    if v in PIXEL_PX:
        return v
    h = _screen_height()
    return 16 if h <= 1600 else 24 if h <= 2200 else 32


def mono_font():
    """Monocraft, если он есть (кэш fontconfig сразу после установки бывает ещё старым — смотрим
    и сами файлы), иначе Terminess из комплекта."""
    r = subprocess.run(["fc-list", "-q", "Monocraft"], capture_output=True)
    dirs = [HOME / ".local/share/fonts", HOME / ".fonts", Path("/usr/share/fonts"), Path("/usr/local/share/fonts")]
    found = r.returncode == 0 or any(d.is_dir() and next(d.rglob("Monocraft*"), None) for d in dirs)
    return "Monocraft" if found else "Terminess Nerd Font Mono"


def _ini_set(path, key, val, section="[Settings]"):
    """key=val в секции section: есть ключ — заменить, нет — дописать в конец своей секции
    (не в конец файла: там чужая секция, и программа его не увидит)."""
    t = path.read_text() if path.exists() else section + "\n"
    if re.search(rf"^{re.escape(key)}=", t, re.M):
        t = re.sub(rf"^{re.escape(key)}=.*$", lambda _: f"{key}={val}", t, flags=re.M)
    elif section in t.splitlines():
        lines = t.splitlines()
        i = lines.index(section) + 1
        while i < len(lines) and not lines[i].startswith("["):
            i += 1
        while i > 0 and not lines[i - 1].strip():
            i -= 1  # перед пустыми строками, что отделяют следующую секцию
        lines.insert(i, f"{key}={val}")
        t = "\n".join(lines) + "\n"
    else:
        t = t.rstrip("\n") + f"\n\n{section}\n{key}={val}\n"
    write(path, t)


def ui_font():
    """(семейство, pt, пиксельный ли) для GTK/Qt: fontUi из настроек шелла — "" Tiny5 по сетке,
    "sans" системный, иначе своё; кегль своего — fontPt (0 — 11)."""
    st = _settings()
    fam = (st.get("fontUi") or "").strip()
    if not fam:
        return UI_FONT, font_px() * 3 // 4, True
    pt = int(st.get("fontPt") or 0) or 11
    return ("Sans" if fam == "sans" else fam), pt, False


def fonts(p=None):
    """GTK 3/4, gsettings, qt6ct, kitty: шрифты из настроек шелла (вкладка «шрифты»).
    Пиксельные — ровно по своей сетке (96 dpi: 16 px = 12 pt), иначе они мылятся."""
    fam, pt, _ = ui_font()
    px = font_px()
    mono = (_settings().get("fontMono") or "").strip() or mono_font()
    mono_pt = px * 3 // 4 if mono in ("Monocraft", "Terminess Nerd Font Mono") else max(pt, 11)
    for v in ("3.0", "4.0"):
        _ini_set(CFG / f"gtk-{v}/settings.ini", "gtk-font-name", f"{fam} {pt}")
    qt = CFG / "qt6ct/qt6ct.conf"
    if qt.exists():
        t = qt.read_text()
        q = lambda f, size: f'"{f},{size},-1,5,400,0,0,0,0,0,0,0,0,0,0,1"'
        t = re.sub(r"^general=.*$", lambda _: f"general={q(fam, pt)}", t, flags=re.M)
        t = re.sub(r"^fixed=.*$", lambda _: f"fixed={q(mono, mono_pt)}", t, flags=re.M)
        write(qt, t)
    if shutil.which("gsettings"):
        for k, v in (("font-name", f"{fam} {pt}"), ("document-font-name", f"{fam} {pt}"),
                     ("monospace-font-name", f"{mono} {mono_pt}")):
            subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", k, v], check=False, capture_output=True)
    write(CFG / "kitty/yami-font.conf", f"# YAMI: written by tools/theme.py (settings → fonts)\nfont_family {mono}\nfont_size {mono_pt}.0\n")


# ── kitty ────────────────────────────────────────────────────────────────────
def kitty(p):
    acc = p["accent"]
    # ANSI-цвета чуть в сторону акцента; на светлом фоне — темнее, иначе пастель не читается
    t = (lambda base, k=0.18: scale_v(mix(base, acc, k), 0.7)) if light(p) else (lambda base, k=0.18: mix(base, acc, k))
    cols = {
        "foreground": p["text"], "background": p["bg"],
        "selection_foreground": hi(p), "selection_background": p["selection"],
        "cursor": p["bright"], "cursor_text_color": p["bg"], "url_color": p["bright"],
        "active_border_color": acc, "inactive_border_color": p["border"], "bell_border_color": p["bright"],
        "visual_bell_color": p["border"],
        "active_tab_foreground": p["onAccent"], "active_tab_background": acc,
        "inactive_tab_foreground": p["textDim"], "inactive_tab_background": p["panel"],
        "tab_bar_background": p["bg"], "mark1_background": acc,
        # в светлой «чёрный» (0) остаётся тёмным: программы пишут им текст
        "color0": p["textDim"] if light(p) else p["panel"], "color8": p["textFaint"],
        "color1": "#b02a22" if light(p) else "#d0352d", "color9": "#d0352d" if light(p) else "#ff5a4c",
        "color2": t("#78b86a"), "color10": t("#a2d692"),
        "color3": t("#d9a03a"), "color11": t("#f0c46a"),
        "color4": t("#5a7fa8"), "color12": t("#86a6c8"),
        "color5": t("#9a5aa0"), "color13": t("#c486c8"),
        "color6": t("#4f9a96"), "color14": t("#7cc2bc"),
        "color7": p["textDim"], "color15": p["text"],
    }
    write(CFG / "kitty/yami-colors.conf",
          "# YAMI: пишет tools/theme.py по палитре шелла\n" +
          "".join(f"{k:<24}{v}\n" for k, v in cols.items()))
    subprocess.run(["pkill", "-USR1", "-x", "kitty"], check=False)


# ── btop ─────────────────────────────────────────────────────────────────────
def btop(p):
    """Тема btop в палитре: графики от тусклого к яркому акценту, рамки — как у панелей."""
    acc, deep, bright = p["accent"], p["deep"], p["bright"]
    th = {
        "main_bg": p["bg"], "main_fg": p["text"], "title": p["text"], "hi_fg": bright,
        "selected_bg": deep, "selected_fg": "#ffffff", "inactive_fg": p["textFaint"],  # на глубоком акценте
        "graph_text": p["textDim"], "meter_bg": p["surfaceHi"], "proc_misc": acc,
        "cpu_box": p["border"], "mem_box": p["border"], "net_box": p["border"], "proc_box": p["border"],
        "div_line": p["border"],
        "temp_start": mix(p["textFaint"], acc, 0.5), "temp_mid": acc, "temp_end": bright,
        "cpu_start": deep, "cpu_mid": acc, "cpu_end": bright,
        "free_start": p["border"], "free_mid": deep, "free_end": acc,
        "cached_start": p["border"], "cached_mid": mix(deep, p["textDim"], 0.5), "cached_end": p["textDim"],
        "available_start": p["border"], "available_mid": deep, "available_end": acc,
        "used_start": deep, "used_mid": acc, "used_end": bright,
        "download_start": p["border"], "download_mid": deep, "download_end": acc,
        "upload_start": p["border"], "upload_mid": mix(acc, p["text"], 0.3), "upload_end": p["text"],
        "process_start": deep, "process_mid": acc, "process_end": bright,
    }
    write(CFG / "btop/themes/yami.theme", "# YAMI: пишет tools/theme.py по палитре шелла\n" +
          "".join(f'theme[{k}]="{v}"\n' for k, v in th.items()))
    conf = CFG / "btop/btop.conf"
    if conf.exists():
        t = conf.read_text()
        if re.search(r'^color_theme\s*=', t, re.M):
            t = re.sub(r'^color_theme\s*=.*$', 'color_theme = "yami"', t, flags=re.M)
        else:
            t += '\ncolor_theme = "yami"\n'
        write(conf, t)
    else:
        write(conf, '#? Config file for btop (YAMI: only the theme is set, btop fills the rest)\ncolor_theme = "yami"\ntheme_background = false\n')


# ── Firefox, LibreWolf, Floorp, Zen: меню, подсказки, панели ─────────────────
def firefox_profiles():
    """Профили из profiles.ini (бывают и вне папки браузера, IsRelative=0) и просто папки с prefs.js."""
    roots = [HOME / ".mozilla/firefox", HOME / ".librewolf", CFG / "librewolf/librewolf", HOME / ".floorp", HOME / ".zen",
             CFG / "mozilla/firefox"]
    # Tor Browser (torbrowser-launcher): профиль внутри скачанного браузера
    roots += sorted((HOME / ".local/share/torbrowser/tbb").glob("*/tor-browser/Browser/TorBrowser/Data/Browser"))
    out = []
    for r in roots:
        if not r.is_dir():
            continue
        cands = [d for d in r.iterdir() if d.is_dir()]
        ini = r / "profiles.ini"
        if ini.exists():
            rel, path = "1", None
            for line in ini.read_text(errors="replace").splitlines() + ["["]:
                line = line.strip()
                if line.startswith("["):
                    if path:
                        cands.append(r / path if rel == "1" else Path(path))
                    rel, path = "1", None
                elif line.startswith("IsRelative="):
                    rel = line.split("=", 1)[1]
                elif line.startswith("Path="):
                    path = line.split("=", 1)[1]
        for d in cands:
            if (d / "prefs.js").exists() and d.resolve() not in [o.resolve() for o in out]:
                out.append(d)
    return out


YAMI_CSS = "/* YAMI:"


def firefox(p):
    """Только меню браузера (ПКМ, выпадающие списки, подсказки): квадратные, в палитре. Тему,
    панели и вкладки не трогаем — они остаются, какими их сделал пользователь. Пиксельный шрифт
    в интерфейсе — по желанию (browserFont). Чистый CSS: ничего не грузит и не замедляет."""
    st = _settings()
    menus = st.get("browserMenus", True)
    font = st.get("browserFont", False)
    px = font_px()
    edge = mix(p['border'], p['accent'], 0.5)
    css = ""
    if menus:
        css += f"""/* YAMI: written by tools/theme.py (settings → fonts → browser). Delete the file to drop it.
   Only the browser's menus: its theme, toolbars and tabs stay as they are. */
:root {{
  --menu-background-color: {p['panel']} !important;
  --menu-color: {p['text']} !important;
  --menu-border-color: {edge} !important;
  --menuitem-hover-background-color: {p['deep']} !important;
  --menuitem-disabled-hover-background-color: transparent !important;
  --menu-disabled-color: {p['textFaint']} !important;
}}
menupopup {{ --panel-padding: 4px !important; --panel-background: {p['panel']} !important; --panel-color: {p['text']} !important; --panel-border-color: {edge} !important; --panel-border-radius: 0px !important; }}
menupopup::part(content) {{
  border-radius: 0 !important;
  border: 1px solid {edge} !important;
  background: {p['panel']} !important;
  box-shadow: 4px 4px 0 {"#00000038" if light(p) else "#000000b4"} !important;
}}
menu, menuitem, menucaption {{
  border-radius: 0 !important;
  padding-block: 2px !important;
  color: {p['text']} !important;
}}
menu[_moz-menuactive], menuitem[_moz-menuactive] {{
  background-color: {p['deep']} !important;
  color: #ffffff !important;
  box-shadow: inset 2px 0 0 {p['bright']} !important;
}}
menu[disabled], menuitem[disabled] {{ color: {p['textFaint']} !important; }}
menuseparator {{ border-top: 1px solid {p['border']} !important; padding-block: 3px !important; }}
tooltip {{
  appearance: none !important;
  background: {p['panel']} !important;
  color: {p['text']} !important;
  border: 1px solid {p['accent']} !important;
  border-radius: 0 !important;
  padding: 3px 7px !important;
}}
"""
    if font:
        css = css or "/* YAMI: written by tools/theme.py (settings → fonts → browser). Delete the file to drop it. */\n"
        css += f"""/* the pixel font exactly on its grid ({px} px), else it blurs; web pages keep their fonts */
#navigator-toolbox, #urlbar, .urlbarView, menupopup, panel, tooltip, #sidebar-box, #sidebar-main,
findbar, #statuspanel-label, .tab-label, .toolbarbutton-text, #identity-box, menu, menuitem {{
  font-family: "{UI_FONT}", "DotGothic16", monospace !important;
  font-size: {px}px !important;
}}
menu, menuitem, menucaption {{ min-height: {px + 8}px !important; }}
"""
    for prof in firefox_profiles():
        f = prof / "chrome/userChrome.css"
        ours = not f.exists() or f.read_text(errors="replace").startswith(YAMI_CSS)
        if not ours:
            continue  # свой userChrome.css пользователя не трогаем
        if not css:
            if f.exists():
                f.unlink()
            continue
        write(f, css)
        uj = prof / "user.js"
        t = uj.read_text() if uj.exists() else ""
        if "toolkit.legacyUserProfileCustomizations.stylesheets" not in t:
            write(uj, t + "// yami: lets the browser read chrome/userChrome.css (menus in the palette)\n"
                  'user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);\n')


# ── fastfetch ────────────────────────────────────────────────────────────────
# 闇 («ями», тьма) — DotGothic16 в родные 16 px, по пикселю
KANJI = [
    "######..######",
    "#....#..#....#",
    "######..######",
    "#....#..#....#",
    "######..######",
    "#.....#......#",
    "#..########..#",
    "#....#..#....#",
    "#.##########.#",
    "#............#",
    "#...######...#",
    "#...#....#...#",
    "#...######...#",
    "#...#....#...#",
    "#...######.###",
    "..............",
]


def kanji_logo(p):
    """Иероглиф полублоками (▀: верхний пиксель — цвет текста, нижний — фон): квадратные
    пиксели, полосы от светлого к акценту сверху вниз и тень на пиксель, как у панелей шелла."""
    w, h = len(KANJI[0]) + 1, len(KANJI) + 1
    bands = [p["bright"], mix(p["bright"], p["accent"], 0.5), p["accent"], mix(p["accent"], p["deep"], 0.5)]
    shadow = mix(p["deep"], p["bg"], 0.45)

    def px(x, y):
        on = lambda x, y: 0 <= y < len(KANJI) and 0 <= x < len(KANJI[0]) and KANJI[y][x] == "#"
        if on(x, y):
            return bands[min(y * len(bands) // len(KANJI), len(bands) - 1)]
        # тень только снаружи (справа и снизу), чтобы не забивать просветы знака
        if on(x - 1, y - 1) and (x >= len(KANJI[0]) or y >= len(KANJI) - 1):
            return shadow
        return None

    rows = []
    for y in range(0, h, 2):
        line = ""
        for x in range(w):
            top, bot = px(x, y), px(x, y + 1)
            if top is None and bot is None:
                line += "\u001b[0m "
            elif bot is None:
                line += "\u001b[0m\u001b[38;2;%d;%d;%dm▀" % rgb(top)
            elif top is None:
                line += "\u001b[0m\u001b[38;2;%d;%d;%dm▄" % rgb(bot)
            else:
                line += "\u001b[38;2;%d;%d;%dm\u001b[48;2;%d;%d;%dm▀" % (rgb(top) + rgb(bot))
        rows.append(line + "\u001b[0m")
    return "\n".join(rows) + "\n"


def fastfetch(p):
    e = lambda h: "38;2;%d;%d;%d" % rgb(h)
    esc = lambda h: "\u001b[%sm" % e(h)
    a, d, f, t = p["accent"], p["border"], p["textFaint"], p["text"]
    logo = kanji_logo(p)
    write(CFG / "fastfetch/yami-logo.txt", logo)
    swatch = "".join("\u001b[48;2;%d;%d;%dm   " % rgb(c) for c in
                     (p["bg"], p["panel"], p["border"], p["deep"], a, p["bright"], p["textDim"], t))
    # мини (по умолчанию): кто, система и окружение — без железа; расширенный (fetchFull): всё.
    # подписи — английские, короткие, в одну колонку
    full = bool(_settings().get("fetchFull", False))
    line = esc(d) + "─" * (44 if full else 30)
    keys = [
        ("title", "host", {"format": "{user-name}@{host-name}"}),
        ("os", "os", {"format": "{pretty-name} {arch}" if full else "{pretty-name}"}),
    ]
    if full:
        keys += [("kernel", "kernel", {"format": "{release}"}), ("uptime", "uptime", {}), ("packages", "pkgs", {})]
    keys += [
        ("wm", "wm", {"format": "{pretty-name}"}),
        ("shell", "shell", {"format": "{pretty-name}"}),
        ("terminal", "term", {"format": "{pretty-name}"}),
    ]
    if full:
        keys += [
            (None, None, None),
            ("cpu", "cpu", {"format": "{name} x{cores-logical}"}),
            ("gpu", "gpu", {"format": "{name}", "hideType": "integrated"}),
            ("memory", "mem", {"format": "{percentage-bar} {used} / {total}"}),
            ("disk", "disk", {"folders": "/", "format": "{size-percentage-bar} {size-used} / {size-total}"}),
            ("display", "display", {"format": "{width}x{height} @ {refresh-rate} Hz"}),
        ]
    else:
        keys += [("uptime", "uptime", {})]
    mods = [{"type": "custom", "format": f"{esc(a)}YAMI {esc(f)}// {esc(p['textDim'])}niri"}, {"type": "custom", "format": line}]
    for typ, key, extra in keys:
        if typ is None:
            mods.append({"type": "custom", "format": line})
            continue
        mods.append({"type": typ, "key": key, **extra})
    mods += ["break", {"type": "custom", "format": swatch + "\u001b[0m"}]
    conf = {
        "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
        "logo": {"type": "file-raw", "source": str(CFG / "fastfetch/yami-logo.txt"),
                 "padding": {"top": 1, "left": 2, "right": 4}},
        "display": {
            "separator": "  ",
            "color": {"keys": e(a), "title": e(p["bright"]), "output": e(t), "separator": e(d)},
            "key": {"width": 9 if full else 8},
            "bar": {"char": {"elapsed": "■", "total": "·"}, "width": 12,
                    "border": {"left": "", "right": ""}},
            "percent": {"type": 3, "color": {"green": e(p["ok"]), "yellow": e(p["warn"]), "red": e(p["bright"])}},
        },
        "modules": mods,
    }
    write(CFG / "fastfetch/config.jsonc", "// YAMI: пишет tools/theme.py по палитре шелла\n" +
          json.dumps(conf, ensure_ascii=False, indent=2) + "\n")


# ── niri: рамки, обзор, Alt+Tab ──────────────────────────────────────────────
def niri(p):
    write(CFG / "niri/yami-colors.kdl", f"""// YAMI: пишет tools/theme.py по палитре шелла. Руками не править.
layout {{
    border {{
        active-color   "{p['accent']}"
        inactive-color "{p['border']}"
        urgent-color   "{p['bright']}"
    }}
    tab-indicator {{
        active-color   "{p['accent']}"
        inactive-color "{p['border']}"
        urgent-color   "{p['bright']}"
    }}
    insert-hint {{
        color "{p['accent']}60"
    }}
}}
overview {{
    backdrop-color "{p['bg']}"
}}
recent-windows {{
    highlight {{
        active-color "{p['accent']}ff"
        urgent-color "{p['bright']}ff"
    }}
}}
""")


# ── MangoHud: игровой оверлей в палитре и пиксельным шрифтом ────────────────────
MANGO_MARK = "# yami: written by tools/theme.py"


def mangohud(p):
    """~/.config/MangoHud/MangoHud.conf в цветах палитры, шрифт Tiny5 по сетке. Свой конфиг
    (без нашей метки) не трогаем."""
    path = CFG / "MangoHud/MangoHud.conf"
    if path.exists() and not path.read_text(errors="replace").startswith(MANGO_MARK):
        return
    # без MangoHud конфиг ему не нужен: папку в ~/.config не заводим
    import shutil
    if not path.exists() and not path.parent.is_dir() and not shutil.which("mangohud"):
        return
    h = lambda k: p[k].lstrip("#")
    font = next((f for f in (HOME / ".local/share/fonts/yami/Tiny5.ttf", HOME / ".local/share/fonts/Tiny5.ttf") if f.exists()), None)
    lines = [
        MANGO_MARK + " (shell palette; delete this line to keep your own edits)",
        "legacy_layout=false",
        "horizontal",
        "hud_compact",
        "round_corners=0",
        "background_alpha=0.7",
        f"background_color={h('bg')}",
        f"text_color={h('text')}",
        f"gpu_color={h('accent')}",
        f"cpu_color={h('bright')}",
        f"vram_color={h('deep')}",
        f"ram_color={h('deep')}",
        f"frametime_color={h('accent')}",
        f"fps_color_change",
        f"fps_color={h('warn')},{h('bright')},{h('ok')}",
        "fps_value=30,60",
        "fps", "frametime", "gpu_stats", "cpu_stats", "vram", "ram", "frame_timing",
        "position=top-left",
        "toggle_hud=Shift_R+F12",
        "font_size=16",
    ]
    if font:
        lines.append(f"font_file={font}")
    write(path, "\n".join(lines) + "\n")


# ── обои: свечение акцента дизерингом Байера клетками 4 px и редкая точечная сетка ─────
def wallpaper_img(p, w=480, h=270):
    """Картинка в четверть экрана — шелл растягивает её ×4 без сглаживания."""
    from PIL import Image
    bayer = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5]
    bg, ac, dot = rgb(p["bg"]), rgb(p["accent"]), rgb(p["border"])
    levels, peak = 7, (0.3 if light(p) else 0.2)   # на бумаге свечение акцента бледнее — сильнее
    r1, r2 = (w * w + h * h) ** 0.5 * 0.75, (w * w + h * h) ** 0.5 * 0.45
    img = Image.new("RGB", (w, h))
    px = []
    for y in range(h):
        for x in range(w):
            a = max(0.0, 1 - ((w - x) ** 2 + (h - y) ** 2) ** 0.5 / r1)
            b = max(0.0, 1 - (x * x + y * y) ** 0.5 / r2)
            g = a * a + b * b * 0.4
            q = int(g * levels + bayer[(y & 3) * 4 + (x & 3)] / 16) / levels * peak
            c = [bg[i] + (ac[i] - bg[i]) * q for i in range(3)]
            if x % 10 == 5 and y % 10 == 5:
                c = [c[i] + (dot[i] - c[i]) * 0.6 for i in range(3)]
            px.append(tuple(round(v) for v in c))
    img.putdata(px)
    return img


def wallpaper(p):
    img = wallpaper_img(p)
    out = HOME / ".local/state/yami/wallpaper.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_name("wallpaper.tmp.png")
    img.save(tmp)
    tmp.replace(out)


# ── курсор: пиксельный, в палитре ────────────────────────────────────────────
# Рисунки на сетке 12×12: o — тёмный контур, a — акцент, b — яркий блик, w — текст.
CURSOR_ART = {
    "default": ((0, 0), [
        "o...........",
        "oo..........",
        "obo.........",
        "oaao........",
        "oaaao.......",
        "oaaaao......",
        "oaaaaao.....",
        "oaaaaaao....",
        "oaaaoooo....",
        "oaoao.......",
        "oo.oao......",
        "o...oo......"]),
    "pointer": ((4, 0), [
        "...oo.......",
        "..obao......",
        "..oaao......",
        "..oaaoooo...",
        "..oaaabaooo.",
        "ooaaaaaaabo.",
        "obaaaaaaaao.",
        ".oaaaaaaaao.",
        "..oaaaaaaao.",
        "..oaaaaaaoo.",
        "...oaaaaao..",
        "...oooooo..."]),
    "text": ((5, 6), [
        "...ooo.ooo..",
        "...owwowwo..",
        "...ooowooo..",
        ".....owo....",
        ".....owo....",
        ".....owo....",
        ".....owo....",
        ".....owo....",
        ".....owo....",
        "...ooowooo..",
        "...owwowwo..",
        "...ooo.ooo.."]),
    "crosshair": ((5, 5), [
        ".....o......",
        ".....a......",
        ".....a......",
        ".....a......",
        "............",
        "oaaa.b.aaao.",
        "............",
        ".....a......",
        ".....a......",
        ".....a......",
        ".....o......",
        "............"]),
    "not-allowed": ((5, 5), [
        "...oooooo...",
        "..oaaaaaao..",
        ".oaaooooaao.",
        "oaaob...oaao",
        "oao.b....oao",
        "oao..b...oao",
        "oao...b..oao",
        "oao....b.oao",
        "oaao...obaao",
        ".oaaooooaao.",
        "..oaaaaaao..",
        "...oooooo..."]),
    "grab": ((5, 5), [
        "...oo.oo....",
        "..oaooaoo...",
        "..oaooaoao..",
        ".ooaooaoaoo.",
        "oaoaaaaaaao.",
        "oaaaaaaaaao.",
        "oaaaaaaaaao.",
        ".oaaaaaaabo.",
        "..oaaaaaao..",
        "..oaaaaaao..",
        "...oooooo...",
        "............"]),
    "grabbing": ((5, 5), [
        "............",
        "............",
        "...oooooo...",
        "..oaoaoaoao.",
        ".ooaaaaaaao.",
        "oaoaaaaaaao.",
        "oaaaaaaaaao.",
        ".oaaaaaaabo.",
        "..oaaaaaao..",
        "..oaaaaaao..",
        "...oooooo...",
        "............"]),
    "move": ((5, 5), [
        ".....o......",
        "....oao.....",
        "...oaaao....",
        ".....a......",
        ".o...a...o..",
        "oaaaabaaaao.",
        ".o...a...o..",
        ".....a......",
        "...oaaao....",
        "....oao.....",
        ".....o......",
        "............"]),
    "ns-resize": ((5, 5), [
        ".....o......",
        "....oao.....",
        "...oaaao....",
        "..ooaaaoo...",
        "....oao.....",
        "....obo.....",
        "....oao.....",
        "..ooaaaoo...",
        "...oaaao....",
        "....oao.....",
        ".....o......",
        "............"]),
    "ew-resize": ((5, 5), [
        "............",
        "............",
        "............",
        "...o...o....",
        "..oo...oo...",
        ".oaoooooao..",
        "oaaaabaaaao.",
        ".oaoooooao..",
        "..oo...oo...",
        "...o...o....",
        "............",
        "............"]),
    "nwse-resize": ((5, 5), [
        "ooooo.......",
        "oaaao.......",
        "oaao........",
        "oaoao.......",
        "oo.oao......",
        "....obo.....",
        ".....oao.oo.",
        "......oaoao.",
        "........oao.",
        ".......oaao.",
        ".......oooo.",
        "............"]),
    "nesw-resize": ((6, 5), [
        ".......ooooo",
        ".......oaaao",
        "........oaao",
        ".......oaoao",
        "......oao.oo",
        ".....obo....",
        ".oo.oao.....",
        ".oaoao......",
        ".oao........",
        ".oaao.......",
        ".oooo.......",
        "............"]),
    "help": ((0, 0), [
        "o.......ooo.",
        "oo.....oaaao",
        "obo....o.oao",
        "oaao.....oao",
        "oaaao...oao.",
        "oaaaao..oo..",
        "oaaaaao.....",
        "oaaaaaao.oo.",
        "oaaaoooo.oo.",
        "oaoao.......",
        "oo.oao......",
        "o...oo......"]),
}
CURSOR_ALIASES = {
    "default": ["left_ptr", "arrow", "top_left_arrow", "context-menu", "copy", "alias", "dnd-copy", "dnd-link", "dnd-ask", "link", "right_ptr", "draft"],
    "pointer": ["hand2", "hand1", "hand", "pointing_hand", "9d800788f1b08800ae810202380a0822", "e29285e634086352946a0e7090d73106", "zoom-in", "zoom-out"],
    "text": ["xterm", "ibeam", "vertical-text"],
    "crosshair": ["cross", "tcross", "cell", "color-picker", "plus", "target"],
    "not-allowed": ["crossed_circle", "no-drop", "forbidden", "dnd-no-drop", "circle", "03b6e0fcb3499374a867c041f52298f0"],
    "grab": ["openhand", "dnd-none", "5aca4d189052212118709018842178c0"],
    "grabbing": ["closedhand", "dnd-move", "208530c400c041818281048008011002"],
    "move": ["all-scroll", "fleur", "size_all", "4498f0e0c1937ffe01fd06f973665830", "9081237383d90e509aa00f00170e968f"],
    "ns-resize": ["n-resize", "s-resize", "row-resize", "size_ver", "sb_v_double_arrow", "v_double_arrow", "top_side", "bottom_side", "split_v", "00008160000006810000408080010102"],
    "ew-resize": ["e-resize", "w-resize", "col-resize", "size_hor", "sb_h_double_arrow", "h_double_arrow", "left_side", "right_side", "split_h", "028006030e0e7ebffc7f7070c0600140"],
    "nwse-resize": ["nw-resize", "se-resize", "size_fdiag", "bd_double_arrow", "top_left_corner", "bottom_right_corner", "c7088f0f3e6c8088236ef8e1e3e70000"],
    "nesw-resize": ["ne-resize", "sw-resize", "size_bdiag", "fd_double_arrow", "top_right_corner", "bottom_left_corner", "fcf1c3c7cd4491d801f1e1c78f100000"],
    "help": ["question_arrow", "whats_this", "left_ptr_help", "5c6cd98b3f3ebcb1f9c7f1c204630408", "d9ce0ab605698f320427677b458ad60b"],
}


def _xcursor(images):
    """images: [(nominal, w, h, xhot, yhot, delay, argb-list)] → байты XCursor"""
    import struct
    n = len(images)
    out = bytearray(struct.pack("<4sIII", b"Xcur", 16, 0x10000, n))
    pos = 16 + n * 12
    chunks = []
    for (nom, w, h, xh, yh, delay, px) in images:
        out += struct.pack("<III", 0xFFFD0002, nom, pos)
        c = struct.pack("<IIIIIIIII", 36, 0xFFFD0002, nom, 1, w, h, xh, yh, delay) + struct.pack(f"<{w*h}I", *px)
        chunks.append(c)
        pos += len(c)
    for c in chunks:
        out += c
    return bytes(out)


def _render(rows, k, pal):
    """сетка 12×12 → ARGB (предумноженный) в масштабе k, с тенью на клетку"""
    size = 12 * k + k
    px = [0] * (size * size)
    def put(x, y, argb):
        for j in range(k):
            for i in range(k):
                px[(y * k + j) * size + x * k + i] = argb
    # тень: сдвиг на клетку вниз-вправо, полупрозрачная
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch != "." and x + 1 < 13 and y + 1 < 13:
                for j in range(k):
                    for i in range(k):
                        q = ((y + 1) * k + j) * size + (x + 1) * k + i
                        if q < len(px) and px[q] == 0:
                            px[q] = 0x50000000
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch in pal:
                put(x, y, pal[ch])
    return size, px


def _argb(h):
    r, g, b = rgb(h)
    return 0xFF000000 | (round(r) << 16) | (round(g) << 8) | round(b)


def cursor_theme(p, d, name):
    """Тема пиксельного курсора в палитре p в папке d (d/cursors, d/index.theme)."""
    if not (d / "cursors/default").exists():
        # тело — цвет текста, обводка — к фону: в светлой теме курсор тёмный со светлым контуром
        edge = p["codeBg"] if light(p) else mix(p["bg"], "#000000", 0.5)
        pal = {"o": _argb(edge), "a": _argb(p["accent"]), "b": _argb(p["bright"]), "w": _argb(p["text"])}
        (d / "cursors").mkdir(parents=True, exist_ok=True)
        for base, (hot, rows) in CURSOR_ART.items():
            imgs = []
            for nom, k in ((24, 2), (32, 3), (48, 4), (64, 5)):
                sz, px = _render(rows, k, pal)
                imgs.append((nom, sz, sz, hot[0] * k, hot[1] * k, 0, px))
            (d / "cursors" / base).write_bytes(_xcursor(imgs))
        # ожидание: восемь точек по кругу, бегущий огонёк — восемь кадров
        ring = [(5, 1), (8, 2), (9, 5), (8, 8), (5, 9), (2, 8), (1, 5), (2, 2)]
        imgs = []
        for nom, k in ((24, 2), (32, 3), (48, 4), (64, 5)):
            for f in range(8):
                rows = [["."] * 12 for _ in range(12)]
                for i, (x, y) in enumerate(ring):
                    ch = "b" if i == f else ("a" if (f - i) % 8 in (1, 2) else "o")
                    for dx, dy in ((0, 0), (1, 0), (0, 1), (1, 1)):
                        rows[y + dy][x + dx] = ch
                sz, px = _render(["".join(r) for r in rows], k, pal)
                imgs.append((nom, sz, sz, 6 * k, 6 * k, 90, px))
        (d / "cursors/wait").write_bytes(_xcursor(imgs))
        aliases = dict(CURSOR_ALIASES)
        aliases["wait"] = ["watch", "progress", "left_ptr_watch", "half-busy", "08e8e1c95fe2fc01f976f1e063a24ccd", "3ecb610c1bf2410f44200f48c40d3599"]
        for base, names in aliases.items():
            for n in names:
                l = d / "cursors" / n
                if not l.exists():
                    l.symlink_to(base)
        write(d / "index.theme", f"[Icon Theme]\nName={name}\nComment=yami pixel cursor ({p['accent']})\nInherits=Adwaita\n")


def cursor(p):
    """Пиксельный курсор в палитре: тема «yami-<акцент>» в ~/.local/share/icons, niri и
    приложения переключаются на неё (~/.config/niri/yami-cursor.kdl). В настройках шелла
    (окна → курсор): пиксельный или системный, размер."""
    st = _settings()
    kind = st.get("cursor", "yami")
    size = int(st.get("cursorSize", 24))
    icons = HOME / ".local/share/icons"
    kdl = CFG / "niri/yami-cursor.kdl"
    if kind == "system":
        if kdl.exists():
            kdl.unlink()
        return
    # имя меняется с цветом и темой: niri перечитывает курсор, только когда сменилось имя
    name = "yami-" + p["accent"].lstrip("#").lower() + ("-l" if light(p) else "")
    cursor_theme(p, icons / name, name)
    # старые цвета — убрать
    for old in icons.glob("yami-*"):
        if old.name != name and re.fullmatch(r"yami-[0-9a-f]{6}(-l)?", old.name) and old.is_dir():
            import shutil
            shutil.rmtree(old, ignore_errors=True)
    write(kdl, f"// yami: written by tools/theme.py (shell settings → windows → cursor)\ncursor {{\n    xcursor-theme \"{name}\"\n    xcursor-size {size}\n}}\nenvironment {{\n    XCURSOR_THEME \"{name}\"\n    XCURSOR_SIZE \"{size}\"\n}}\n")
    # X11-программам (xwayland-satellite) — тема по умолчанию
    write(icons / "default/index.theme", f"[Icon Theme]\nName=default\nInherits={name}\n")
    for v in ("3.0", "4.0"):
        if (CFG / f"gtk-{v}/settings.ini").exists():
            _ini_set(CFG / f"gtk-{v}/settings.ini", "gtk-cursor-theme-name", name)
            _ini_set(CFG / f"gtk-{v}/settings.ini", "gtk-cursor-theme-size", str(size))
    if shutil.which("gsettings"):
        subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "cursor-theme", name], check=False, capture_output=True)
        subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "cursor-size", str(size)], check=False, capture_output=True)


def _sddm_dir():
    """Тема SDDM: первая sddm/themes/yami в XDG_DATA_DIRS (так и на NixOS), иначе /usr/share."""
    for d in (os.environ.get("XDG_DATA_DIRS") or "/usr/local/share:/usr/share").split(":"):
        if d.startswith("/") and (Path(d) / "sddm/themes/yami").is_dir():
            return Path(d) / "sddm/themes/yami"
    return Path("/usr/share/sddm/themes/yami")


SDDM_DIR = _sddm_dir()


def sddm(p, target=None):
    """Экран входа: цвета в theme.conf.user, обои шелла — background.png рядом с темой
    (приветствие работает от пользователя sddm и в домашнюю папку не заглядывает)."""
    d = Path(target) if target else SDDM_DIR
    user = d / "theme.conf.user"
    if not target and not (d.is_dir() and os.access(user if user.exists() else d, os.W_OK)):
        return
    s = _settings()
    keys = ("accent", "deep", "bright", "bg", "panel", "surface", "surfaceHi", "codeBg", "border",
            "text", "textDim", "textFaint", "onAccent", "textHi", "warn", "mode")
    lines = ["# yami: written by tools/theme.py from the shell palette", "[General]"]
    lines += [f"{k}={p[k]}" for k in keys if k in p]
    lines.append(f"lang={s.get('lang', '')}")
    own = s.get("wallpaper") or ""
    # свои обои — уже подготовленные шеллом (обрезка, клетки, окраска, затемнение: wall.rs)
    ready = HOME / ".local/state/yami/wallpaper-own.png"
    src = (ready if ready.is_file() else Path(own).expanduser()) if own else HOME / ".local/state/yami/wallpaper.png"
    if src.is_file():
        bg = d / "background.png"
        try:
            from PIL import Image
            img = Image.open(src)
            # своя картинка — не больше 2560 по ширине: приветствию хватит
            if img.width > 2560:
                img = img.resize((2560, round(img.height * 2560 / img.width)))
            img.convert("RGB").save(bg)
        except Exception:
            bg.write_bytes(src.read_bytes())
        # сгенерированные обои — в четверть экрана: растягивать без сглаживания
        lines.append(f"pixelate={'false' if own and s.get('wallPixel', 1) <= 1 else 'true'}")
    lines.append(f"kanji={'true' if s.get('wallKanji', True) else 'false'}")
    write(user, "\n".join(lines) + "\n")


def main():
    args = sys.argv[1:]
    if args[:1] == ["--cursor"]:
        path = HOME / ".local/state/yami/palette.json"
        cursor(json.loads(path.read_text()) if path.exists() else derive("#c4161c"))
        return
    if args[:1] == ["--cursor-to"] and len(args) > 1:
        # экрану входа (SDDM): тема курсора «yami-cursor» в папку args[1], её копируют в /usr/share/icons
        path = HOME / ".local/state/yami/palette.json"
        p = json.loads(path.read_text()) if path.exists() else derive("#c4161c")
        cursor_theme(p, Path(args[1]), "yami-cursor")
        return
    if args[:1] == ["--profiles"]:
        # профили Firefox-семейства (для установщика), по одному на строку
        for d in firefox_profiles():
            print(d)
        return
    if args[:1] == ["--fonts"]:
        path = HOME / ".local/state/yami/palette.json"
        p = json.loads(path.read_text()) if path.exists() else derive("#c4161c")
        for step in (fonts, firefox):
            step(p)
        subprocess.run(["pkill", "-USR1", "-x", "kitty"], check=False)
        return
    if args[:1] == ["--fetch"]:
        # шелл переключил вид fastfetch (настройки → вид): только его конфиг
        path = HOME / ".local/state/yami/palette.json"
        fastfetch(json.loads(path.read_text()) if path.exists() else derive("#c4161c"))
        return
    if args[:1] == ["--login"]:
        # шелл сменил обои: только экран входа (если тема SDDM открыта на запись)
        path = HOME / ".local/state/yami/palette.json"
        sddm(json.loads(path.read_text()) if path.exists() else derive("#c4161c"))
        return
    if args[:1] == ["--sddm"] and len(args) > 1:
        path = HOME / ".local/state/yami/palette.json"
        p = json.loads(path.read_text()) if path.exists() else derive("#c4161c")
        sddm(p, args[1])
        return
    if args[:1] == ["--accent"] and len(args) > 1:
        # --accent '#rrggbb' [tint] [--light]
        rest = [a for a in args[2:] if a != "--light"]
        p = derive(args[1], float(rest[0]) if rest else 1.0, "--light" in args)
    else:
        path = Path(args[0]) if args else HOME / ".local/state/yami/palette.json"
        p = json.loads(path.read_text()) if path.exists() else derive("#c4161c")
    for step in (wallpaper, qt6ct, kde, gtk, fonts, kitty, btop, mangohud, firefox, fastfetch, niri, cursor, sddm):
        try:
            step(p)
        except Exception as exc:  # одна программа не должна ломать остальные
            print(f"theme: {step.__name__}: {exc}", file=sys.stderr)


if __name__ == "__main__":
    main()
