#!/usr/bin/env bash
# YAMI: fonts and style for every app: Qt (qt6ct), GTK 3/4, gsettings.
# Colors come from tools/theme.py following the shell palette (settings → color); this is the
# first run. Every file it touches is saved next to itself once as *.bak-yami. Only what is chosen
# to restyle: YAMI_RESTYLE (the installer: the themeApps list, gtk and qt here), all when unset.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
THEME="$ROOT/tools/theme.py"
CFG="${XDG_CONFIG_HOME:-$HOME/.config}"
UI_FONT="Tiny5"
UI_PT=12          # 12 pt = 16 px at 96 dpi: Tiny5's grid, not a single blurry pixel
MONO_FONT="Monocraft"
fc-list -q Monocraft 2>/dev/null || MONO_FONT="Terminess Nerd Font Mono"
# icons: the set chosen in the shell (iconTheme), else the one GTK already uses, else Adwaita; yami
# installs no icon set of its own (its apps draw their icons in the palette, theme.py)
ICONS=$(python3 -I -c 'import json,sys
try: print(json.load(open(sys.argv[1])).get("iconTheme") or "")
except Exception: print("")' "$CFG/yami-shell.json" 2>/dev/null)
[[ -n $ICONS ]] || ICONS=$(sed -n 's/^gtk-icon-theme-name=//p' "$CFG/gtk-3.0/settings.ini" 2>/dev/null | head -1 || true)
[[ -n $ICONS ]] || ICONS=$(gsettings get org.gnome.desktop.interface icon-theme 2>/dev/null | tr -d "'" || true)
[[ $ICONS =~ ^[A-Za-z0-9._+\ -]{1,64}$ ]] || ICONS=Adwaita

keep() { [[ -e "$1" && ! -e "$1.bak-yami" ]] && cp -a -- "$1" "$1.bak-yami"; return 0; }
want() { [[ -z ${YAMI_RESTYLE+x} || " $YAMI_RESTYLE " == *" $1 "* ]]; }

# light or dark base for GTK (settings → color → theme); theme.py switches it later on its own
PALETTE="${YAMI_PALETTE:-${XDG_STATE_HOME:-$HOME/.local/state}/yami/palette.json}"
GTK_THEME=adw-gtk3-dark PREFER_DARK=true SCHEME=prefer-dark
if grep -qs '"mode": "light"' "$PALETTE"; then
  GTK_THEME=adw-gtk3 PREFER_DARK=false SCHEME=prefer-light
fi

# ── colors: back up the old files, then run the generator ──────────────────
want qt && keep "$CFG/kdeglobals"
if want gtk; then for v in 3.0 4.0; do keep "$CFG/gtk-$v/gtk.css"; done; fi
want fastfetch && [[ -f "$CFG/fastfetch/config.jsonc" ]] && ! grep -qE "YAMI|DARK-4RMY" "$CFG/fastfetch/config.jsonc" && keep "$CFG/fastfetch/config.jsonc"
python3 -I "$THEME" "$PALETTE"

# ── Qt: qt6ct with Fusion, palette, QSS, font ──────────────────────────────
if want qt; then
mkdir -p "$CFG/qt6ct"
keep "$CFG/qt6ct/qt6ct.conf"
cat >"$CFG/qt6ct/qt6ct.conf" <<EOF
[Appearance]
color_scheme_path=$CFG/qt6ct/colors/yami.conf
custom_palette=true
icon_theme=$ICONS
standard_dialogs=xdgdesktopportal
style=Fusion

[Fonts]
fixed="$MONO_FONT,$UI_PT,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"
general="$UI_FONT,$UI_PT,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"

[Interface]
activate_item_on_single_click=1
buttonbox_layout=0
cursor_flash_time=1000
dialog_buttons_have_icons=1
double_click_interval=400
keyboard_scheme=2
menus_have_icons=true
show_shortcuts_in_context_menus=true
stylesheets=$CFG/qt6ct/qss/yami.qss
toolbutton_style=4
underline_shortcut=1
wheel_scroll_lines=3

[Troubleshooting]
force_raster_widgets=1
EOF
fi

# ── GTK 3/4: adw-gtk3(-dark) and libadwaita, colors from the generated gtk.css ─
want gtk && for v in 3.0 4.0; do
  mkdir -p "$CFG/gtk-$v"
  ini="$CFG/gtk-$v/settings.ini"
  keep "$ini"
  [[ -f "$ini" ]] || printf '[Settings]\n' >"$ini"
  for kv in "gtk-font-name=$UI_FONT $UI_PT" "gtk-theme-name=$GTK_THEME" \
    "gtk-application-prefer-dark-theme=$PREFER_DARK" "gtk-icon-theme-name=$ICONS"; do
    k="${kv%%=*}"
    if grep -q "^$k=" "$ini"; then sed -i "s|^$k=.*|$kv|" "$ini"; else printf '%s\n' "$kv" >>"$ini"; fi
  done
done

# ── gsettings: read by GTK4 apps and the portal under niri ─────────────────
if want gtk && command -v gsettings >/dev/null; then
  s() { gsettings set org.gnome.desktop.interface "$1" "$2" 2>/dev/null || true; }
  s gtk-theme "$GTK_THEME"
  s color-scheme "$SCHEME"
  s font-name "$UI_FONT $UI_PT"
  s document-font-name "$UI_FONT $UI_PT"
  s monospace-font-name "$MONO_FONT $UI_PT"
  s icon-theme "$ICONS"
fi
# the pixel cursor in the palette (GTK, gsettings, niri, X11 apps)
python3 -I "$THEME" --cursor
# font sizes on the pixel grid for this screen (also re-run by the shell when screens change)
python3 -I "$THEME" --fonts
case "${LANG:-}" in
  ru*) echo "тема применена: Qt, KDE, GTK 3/4. Открытые программы подхватят её после перезапуска." ;;
  *) echo "theme applied: Qt, KDE, GTK 3/4. Running apps pick it up after a restart." ;;
esac
