# yami // niri

**English** · [Русский](README.ru.md)

**yami** (闇, "darkness") is a pixel-art desktop for the [niri](https://github.com/YaLTeR/niri) Wayland compositor: dark
graphite, a single accent color, crisp unantialiased pixel fonts. The rice ships with
**yami-shell**, a native Wayland shell written in Rust that provides the bar, launcher,
notifications, lock screen, desktop widgets and settings in a single ~6 MB binary using
**about 18 MB of RAM**.

## Highlights

- **Native and lightweight.** Software rendering into shared memory through layer-shell; no
  GPU context, no toolkit, no JavaScript. Fonts are memory-mapped, and redraws only happen when
  something changes.
- **Complete desktop shell.** Bar, app launcher with clipboard history, power menu, lock screen
  (PAM), notification server with history and do-not-disturb, OSD, system tray (StatusNotifierItem
  with DBusMenu), calendar, sound/network/Bluetooth/system popups, a power profile switch
  (power-profiles-daemon) and its own **polkit password window** instead of a Qt/GTK agent.
- **Desktop widgets.** Clock, calendar, music card with an audio-reactive equalizer, and graphs
  for CPU, RAM, temperature and rx/tx network traffic in five styles (bars, line, blocks, wave,
  dots) and four animations. Drag them into place in edit mode, hide or resize them.
- **Pixel dock.** Pinned apps and open windows on any edge, window markers, a hover animation to
  choose (grow, wave, lift or none) that steps in whole pixels, a stepped bounce on launch; never, smart (empty workspace) or always hidden.
- **Screenshots.** `Print` freezes the screen under a pixel overlay: area, window or monitor, a
  loupe with a colour picker, pen, frame, arrow, marker, mosaic and numbered badges; copies and/or
  saves, the notification has a thumbnail and open / show-in-folder.
- **yami-files.** A file manager in the same pixels: tabs, split view, icons and details, places,
  devices (udisks2), freedesktop trash, copy/move with progress and conflicts, undo, drag and
  drop, thumbnails, open with. It is also the **file dialog** of every app that uses the portal
  (Firefox, LibreWolf, Chromium, Telegram, Electron, GTK/KDE apps) and answers **show in folder**.
- **Screen sharing.** OBS, Discord/Equibop and browsers get a pixel «what to share» window from the
  shell (monitors and windows); the stream itself comes from niri, through the yami portal.
- **yami-view.** A fast image viewer and quick editor (rotate, flip, crop, save as, wallpaper),
  real pixels when zoomed, GIF/WebP animation. Photo viewers open as floating windows.
- **Screens.** Settings → screens: drag monitors into place, pick resolution, refresh rate, scale,
  rotation and VRR; applied through `~/.config/niri/yami-outputs.kdl`, reverted after 15 s unless kept.
- **Login screen.** An SDDM theme that matches the lock screen and follows the palette.
- **Desk mode.** Toggle niri between tiling and a classic floating desktop with DE-style title
  bars, minimize/restore and a task list in the bar.
- **yami-niri.** Our niri: upstream 26.04 plus a short patch series, built for your CPU (fat
  LTO, `target-cpu=native`): tearing for fullscreen games (async page flips, less input lag), a
  realtime compositor thread so a game loading every core doesn't make the desktop stutter. The
  packaged niri stays as an automatic fallback. See [fork/README.md](fork/README.md).
- **Games.** Settings → games: game windows open fullscreen with VRR (and tearing on yami-niri),
  a big shader cache; while a game has focus notifications wait quietly and the power profile
  switches to «performance»; the shell stops redrawing what the game covers.
- **Every key rebindable.** Settings → keys: all niri and shell bindings by category, press the
  new keys right there (even ones niri itself uses), conflicts are freed and named, add your own
  commands. Written to `~/.config/niri/yami-binds.kdl`.
- **One palette everywhere.** Seven presets or any custom accent recolor the shell, wallpaper,
  niri borders, kitty, btop, fastfetch (闇 logo), Qt/KDE and GTK apps, the browser's menus (its
  own theme stays yours), the pixel cursor and the login screen at once. App icons in the launcher, dock and file manager are
  redrawn as 16×16 pixel art in the palette tones (can be turned off).
- **Qt and KDE apps look like the shell.** Notched buttons, fields, checkboxes and tabs drawn from
  the palette (qt6ct + QSS, KDE apps kept on Fusion instead of Breeze), and their file dialogs open
  in yami-files through the portal, like GTK apps and browsers (Tor Browser included).
- **A curtain for streams.** Mod+Shift+H: viewers of a stream, a Discord demo or an OBS recording
  see the frozen screen blurred (or in big pixels, or just the wallpaper), while you keep your
  desktop and a small «hidden» badge. Settings → screenshots. Needs yami-niri
  (`show-only-in`); on other niri the windows turn black in the stream instead.
- **Your wallpaper.** Settings → wallpaper: pick an image (or «set as wallpaper» in yami-files and
  yami-view), fill or fit, pixel cells, recolour it into the accent or the palette tones, darken
  it, or let the palette follow the image. The login screen gets the same picture.
- **Crisp on any screen.** The shell, yami-files and yami-view render at the monitor's real scale
  (fractional-scale + viewporter) and upscale their pixels by nearest neighbour, so a 4K screen at
  scale 2 shows sharp 2×2 pixels instead of a compositor-smeared image. Tiny5 is drawn on its 8 px
  grid with no antialiasing (16/24/32 px by screen size).
- **Your fonts.** Settings → fonts: any font for the shell, separately for yami-files and
  yami-view, for GTK/Qt apps (readable sans by default, or pixel everywhere) and the terminal.
- **Pixel motion.** Panels and windows dissolve in on a Bayer dither grid, matching niri's custom
  window-open shaders.
- **Bilingual.** English and Russian interface, switchable live in settings.
- **Portable installer.** One script for Arch, Fedora (including atomic), Debian/Ubuntu,
  openSUSE, Void, Alpine, Gentoo and NixOS.

## Installation

One command on a fresh system (it fetches the repository into `~/.local/share/yami/src` and runs
the installer from there):

```sh
bash <(curl -fsSL https://raw.githubusercontent.com/Trendorin/yami/main/install.sh)
```

or from a checkout: `git clone https://github.com/Trendorin/yami yami && yami/install.sh`.
The installer is interactive: language, components, palette, panels, app font, then a summary
before anything changes. Reboot afterwards (or log out) and pick the **Niri** session: you land
in the finished desktop. Useful flags:

| Flag | Effect |
|---|---|
| `--yes` | no questions, defaults for this machine |
| `--lang en\|ru` | language of the installer, the shell and the key hints |
| `--palette ID` | `blood` `ash` `sakura` `amber` `venom` `ice` `dusk` |
| `--look ID` | panels: `matte` `glass` `light` `solid` |
| `--font sans\|pixel` | app font: readable sans (default) or the pixel Tiny5 everywhere |
| `--no-fork` | keep the packaged niri instead of building yami-niri |
| `--login`, `--no-login` | SDDM with the yami theme even if another login manager is set / leave it alone |
| `--no-sudo` | home directory only: no packages, no system files |
| `--dry-run` | print the plan, change nothing |
| `--uninstall` | remove the shell, the fork and the dotfiles, restore the backups |
| `--no-anim` | disable the installer animations |

What it does:

1. Detects the distribution from `/etc/os-release` and installs packages with its native package
   manager (niri, portals, polkit, PipeWire, NetworkManager, BlueZ, power-profiles-daemon, Noto
   fonts, cliphist, grim/slurp, kitty, fastfetch, btop, udisks2, ffmpegthumbnailer, bsdtar, qt6ct,
   adw-gtk3, Papirus, SDDM). Names a repository does not carry are reported, not fatal. Enables
   Bluetooth and power profiles, and NetworkManager when nothing else manages the network.
2. Builds the workspace with `cargo build --release --locked` (Rust ≥ 1.89; rustup is used
   when the distribution's toolchain is older) and installs `yami-shell`, `yami-files` and
   `yami-view` into `~/.local/bin`; registers yami-files for folders and as the portal file
   dialog / FileManager1, yami-view for images, and the Firefox-family portal pref.
3. Installs the pixel fonts (Tiny5, DotGothic16, Terminess, Monocraft — pinned and checksummed)
   with fontconfig rules and the pixel cursor in the palette.
4. Writes the niri, kitty and fastfetch configuration, `yami-local.kdl` for this machine's GPU,
   the keybindings and the game rules. Existing files are moved to `*.bak-DATE`; `output` blocks
   from a previous niri config are carried over into `user.kdl`.
5. Recolours everything around the shell (Qt, KDE, GTK, kitty, btop, browser menus, cursor).
6. With the «login screen» component: installs the SDDM theme (with the pixel cursor) and makes
   SDDM the login manager from the next boot (off by default when another one is set).
7. Builds yami-niri (5–25 minutes, output shown) and enables it from the next login.

An existing install under the old name DARK-4RMY is moved over: settings, palette, widgets
and clipboard pins are kept, the old files are removed; the old dark-niri fork is removed too.
Everything is logged to `~/.cache/yami/install.log`. On NixOS the installer sets up the dotfiles
and the shell and prints the packages to add to `configuration.nix`.

### Building the shell by hand

```sh
cd shell
cargo build --release --locked
install -Dm755 target/release/yami-shell ~/.local/bin/yami-shell
```

Runtime libraries: `libxkbcommon`, `libpam`. Optional helpers: `pactl`/`parec` (volume, mixer,
equalizer), `playerctl` (media), `nmcli` (Wi-Fi), BlueZ (Bluetooth), `cliphist` and
`wl-clipboard` (clipboard), `python3` with Pillow (palette export to other apps).

## Resource usage

Measured on this setup (1920×1080, Arch, PSS — shared library pages split fairly; RSS in brackets):

| What | Memory |
|---|---|
| yami-shell, idle (bar, dock, widgets, notifications, tray) | 15 MB (21 MB) |
| yami-shell with the settings open | 18 MB |
| yami-shell during a screenshot (frozen frame), freed on close | 32 MB → 16 MB |
| yami-files --portal (file dialog + FileManager1 service, always on) | 5 MB (7 MB) |
| yami-files window on the home folder | 12 MB (21 MB) — Dolphin: 69 MB (161 MB) |
| yami-view on a 1080p PNG, both neighbours decoded ahead | 46 MB (56 MB) — Gwenview: 96 MB (188 MB) |
| binaries on disk: shell + files + view | 6.3 + 5.5 + 2.2 MB |

Always running: about 20 MB for the shell and the portal together. No GPU context, no toolkit,
no interpreter; windows are drawn into shared memory and only when something changes.

## Keys

| Keys | Action |
|---|---|
| `Mod+Space`, `Mod+D` | launcher (`Tab` switches to the clipboard) |
| `Mod+V` | clipboard: `Ctrl+P` pin, `Del` delete |
| `Mod+S` | shell settings |
| `Mod+T` | kitty |
| `Mod+Shift+Space` | desk mode ↔ tiling |
| `Mod+N`, `Mod+Shift+N` | minimize window, restore the last one |
| `Mod+Q`, `Alt+F4` | close window |
| `Mod+F`, `Mod+Shift+F` | maximize column, fullscreen |
| `Mod+G` | toggle floating |
| `Mod+Tab` | overview |
| `Alt+Tab`, ``Alt+` `` | recent windows, windows of the same app |
| `Mod+1…9`, `Mod+Shift+1…9` | focus workspace, move column there |
| `Mod+arrows`, `Mod+HJKL` | focus; with `Ctrl` move |
| `Print`, `Mod+Shift+S` | screenshot overlay; `Ctrl+Print` monitor, `Alt+Print` window |
| `Mod+E` | yami-files |
| `Mod+Escape` | lock |
| `Mod+Shift+E`, `Ctrl+Alt+Del` | power menu |
| `Mod+B` | hide the bar |
| `Mod+Shift+/` | all key bindings |

These are the defaults: change any of them in settings → keys (`yami-shell binds list` prints the
current set).

In the bar: left click on a task focuses, minimizes and restores it, middle click closes, right
click opens the window menu; scroll over workspaces to switch, over the volume to change it.

## Configuration

Everything in settings (`Mod+S`) is also a key in `~/.config/yami-shell.json`, reloaded live when
edited by hand; one bad value falls back to its default instead of resetting the file. The full
reference is [docs/CONFIG.md](docs/CONFIG.md). From the terminal:

```sh
yami-shell config list                 # every key, * marks what differs from the default
yami-shell config set dockSide '"left"'
yami-shell config set binds '{"terminal": "Mod+Return"}'
yami-shell config check                # bad values and unknown keys
yami-shell binds list                  # keybindings: id, keys, action
```

- **Settings tabs**: color, bar (drag module chips and separators), widgets, look, fonts, screens,
  dock, windows (floating rules, cursor), screenshots, games, keys, system.
- **niri**: the shell owns `yami-binds.kdl`, `yami-fx.kdl` (window rules, blur, games),
  `yami-colors.kdl`, `yami-cursor.kdl` and `yami-outputs.kdl`; `~/.config/niri/user.kdl` is
  included after them and never overwritten, so your binds, rules and outputs there win.
- **IPC**: `yami-shell msg <command>`, where the command is one of `launcher`, `clipboard`,
  `power`, `settings`, `lock`, `bar`, `stacking`, `minimize`, `restore`, `edit`, `reload`,
  `quit`, `profile` (next power profile), `popup cal|sound|net|bt|sys|profile`, `screenshot [area|window|screen]`,
  `wallpaper [PATH]` (no path: the generated pixel wallpaper), `curtain [on|off]` (privacy curtain).
- **yami-files**: `~/.config/yami-files.json` (also from its ≡ menu → settings). `yami-files
  --pick open|save|dir …` is the dialog the portal runs; `yami-files --portal` is the portal and
  FileManager1 service, started by D-Bus.
- **Login screen**: `tools/sddm-install.sh` installs the theme on a running system and makes SDDM
  the login manager from the next boot; the installer does it with the «login screen» component.

## Repository layout

| Path | Contents |
|---|---|
| `shell/` | yami-shell, the Rust Wayland shell (cargo workspace) |
| `shell/px/` | yami-px: canvas, fonts, theme, icons and UI shared by every program |
| `shell/files/` | yami-files: file manager, portal file dialog, FileManager1 |
| `shell/view/` | yami-view: image viewer and quick editor |
| `sddm/yami/` | SDDM login theme (Qt 6 QML, dissolve shader) |
| `config/portal/`, `config/applications/` | portal registration, D-Bus services, desktop entries |
| `config/niri/` | niri configuration (keybindings come from the shell) |
| `config/kitty/`, `config/fontconfig/` | terminal and pixel font rendering |
| `tools/theme.py` | palette export: Qt, KDE, GTK, kitty, fastfetch, niri, wallpaper |
| `tools/apply-theme.sh` | fonts and widget style for Qt and GTK |
| `fonts/` | bundled pixel fonts (SIL OFL) |
| `fork/` | yami-niri: patch series on upstream niri, build script, session wrapper |
| `docs/` | configuration reference, security notes |
| `system/nvidia/` | driver profile against VRAM growth in the compositor; modeset options for early KMS (the installer also puts the driver into the initramfs, and on Pascal-era drivers adds obs-gstreamer: OBS 33 needs NVENC API 13.1, driver 610+) |

## yami-niri

Upstream niri v26.04 plus the patches in `fork/patches/`, built on your machine. The installer
builds and enables it; by hand:

```sh
fork/yami-niri.sh build     # fetch, patch, build (5–25 min), enable from the next login
fork/yami-niri.sh status    # which niri the next login starts
fork/yami-niri.sh off       # back to the packaged niri
```

The session starts it through a wrapper that falls back to the packaged niri when the fork rejects
the config or dies right after the start. If the session still does not come up: `Ctrl+Alt+F3`,
log in, `~/.local/lib/yami-niri/yami-niri.sh off`. Details: [fork/README.md](fork/README.md).

## Development

| Command | Does |
|---|---|
| `tools/check.sh` | build without warnings, i18n checks, script syntax, niri config, installer dry run (CI runs it too) |
| `tools/map.sh [TEXT]` | every source file with its one-line description |
| `yami-shell shot [TARGET…] --lang en -o DIR` | render panels to PNG without a compositor; fails on untranslated text |
| `yami-files shot`, `yami-view shot` | the same for the file manager and the viewer |
| `tools/sddm-preview.sh OUT.png` | render the login screen offscreen with your palette (`--window` to try it) |
| `tools/deploy.sh` | build, install to `~/.local/bin` and restart the running shell (refuses while a game has focus); `--files-only` installs yami-files and yami-view only |

## License

Code: MIT, see [LICENSE](LICENSE). Bundled fonts keep their own licenses (SIL Open Font
License, texts in `fonts/`). Monocraft is downloaded from its upstream release at install time
(pinned version, checked against its SHA-256). yami-niri is built from upstream niri (GPL-3.0)
with the patches in `fork/`.
