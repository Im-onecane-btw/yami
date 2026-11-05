<div align="center">

<img src=".github/assets/logo.svg" width="88" alt="">

# yami

A pixel-art desktop for niri, drawn in whole pixels.

[![Release](https://img.shields.io/github/v/release/Trendorin/yami?label=release&color=a8a29c)](https://github.com/Trendorin/yami/releases/latest)
[![Installs](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FTrendorin%2Fyami%2Fstats%2Fbadge.json)](docs/SECURITY.md#install-count)
[![License](https://img.shields.io/badge/license-MIT-a8a29c)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.89%2B-a8a29c)
![niri](https://img.shields.io/badge/niri-26.04-a8a29c)
![Platform](https://img.shields.io/badge/platform-Linux%20%C2%B7%20Wayland-a8a29c)

[Install](#installation) · [Screenshots](#screenshots) · [Settings](docs/CONFIG.md) · [Changelog](CHANGELOG.md) · [Русский](README.ru.md)

</div>

https://github.com/user-attachments/assets/1bdd1250-d04c-4993-b66f-0dc29b4512a5

<img src=".github/assets/clean.png" width="100%" alt="The yami desktop as it is: the bar, the clock, CPU and music widgets on the pixel wallpaper">

**yami** (闇, "darkness") turns the [niri](https://github.com/YaLTeR/niri) Wayland compositor into a
finished desktop: dark graphite, one accent colour, crisp unantialiased pixel fonts. Its own
shell, **yami-shell**, draws the bar, launcher, dock, notifications, lock screen, widgets and
settings itself, in Rust and shared memory, in one ~6 MB binary that uses **about 15 MB of RAM**.
Files, images, the login screen and every other app follow the same palette.

<img src=".github/assets/features-en.png" width="100%" alt="What is inside: a ~15 MB shell, one accent for everything, made for games, files and viewer, screenshots, a curtain for streams, signed updates, any distribution">

Install on any of eight distributions with one command, built from source:

```sh
bash <(curl -fsSL https://raw.githubusercontent.com/Trendorin/yami/main/install.sh)
```

Everything it does and its flags: [Installation](#installation).

## Features

- **Native and lightweight.** Software rendering into shared memory through layer-shell; no
  GPU context, no toolkit, no JavaScript. Fonts are memory-mapped, and redraws only happen when
  something changes.
- **Complete desktop shell.** Bar, app launcher with clipboard history, power menu, lock screen
  (PAM), notification server with history and do-not-disturb, OSD, system tray (StatusNotifierItem
  with DBusMenu), calendar, sound/music/network/Bluetooth/system popups, a power profile switch
  (power-profiles-daemon) and its own **polkit password window** instead of a Qt/GTK agent.
- **Desktop widgets.** Clock, calendar, music card with an audio-reactive equalizer, and graphs
  for CPU, RAM, temperature and rx/tx network traffic in five styles (bars, line, blocks, wave,
  dots) and four animations. Drag them into place in edit mode, hide or resize them. Widgets
  without a frame follow the wallpaper under them: dark text on a light spot, light on a dark one.
- **Pixel dock.** Pinned apps and open windows on any edge, window markers, a hover animation to
  choose (grow, wave, lift or none) that steps in whole pixels, a stepped bounce on launch; never, smart (empty workspace) or always hidden.
- **Screenshots.** `Print` freezes the screen under a pixel overlay: area, window or monitor, a
  loupe, pen, frame, arrow, marker, mosaic and numbered badges, a colour picker that copies hex,
  text recognition (tesseract) and QR codes (zbarimg) from the selection; copies
  and/or saves, the notification has a thumbnail and open / show-in-folder.
- **yami-files.** A file manager in the same pixels: tabs, split view, icons and details, places,
  devices (drives through udisks2, phones and cameras over MTP/PTP, iPhone), network shares
  (smb, sftp, ftp, WebDAV, nfs: «connect to a server», addresses remembered without passwords), freedesktop trash, copy/move with progress and conflicts, undo, drag and
  drop, thumbnails, open with. It is also the **file dialog** of every app that uses the portal
  (Firefox, LibreWolf, Chromium, Telegram, Electron, GTK/KDE apps) and answers **show in folder**.
- **Screen sharing.** OBS, Discord/Equibop and browsers get a pixel «what to share» window from the
  shell (monitors and windows); the stream itself comes from niri, through the yami portal.
- **yami-view.** A fast image viewer and quick editor (rotate, flip, crop, save as, wallpaper),
  real pixels when zoomed, GIF/WebP animation. Photo viewers open as floating windows.
- **yami-play.** A video and music player in the same pixels: every format ffmpeg knows,
  hardware decoding, subtitles and audio tracks, a cell timeline, folder playlist in natural
  order, resume where you stopped, speed, frame step, saving a frame, a «pixel» scaling mode.
  libmpv is loaded at run time (the only dependency); the screen stays on while a video plays.
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
  switches to «performance»; the shell stops redrawing what the game covers. It finds and uses what
  the system has on any distro: ntsync for Proton, GameMode for the focused game, MangoHud in the
  palette (`yami doctor games` lists it all).
- **Every key rebindable.** Settings → keys: all niri and shell bindings by category, press the
  new keys right there (even ones niri itself uses), conflicts are freed and named, add your own
  commands. Keyboard layouts too: by default they follow the system languages (an English system
  gets `us`, a Russian one `us,ru`), any xkb codes and the switch combination are a choice. Written
  to `~/.config/niri/yami-binds.kdl`.
- **One palette everywhere.** Seven presets or any custom accent recolor the shell, wallpaper,
  niri borders, kitty, btop, fastfetch (闇 logo), Qt/KDE and GTK apps, the browser's menus (its
  own theme stays yours), the pixel cursor and the login screen at once. App icons in the launcher, dock and file manager are
  redrawn as 16×16 pixel art in the palette tones (can be turned off).
- **Qt and KDE apps look like the shell.** Notched buttons, fields, checkboxes and tabs drawn from
  the palette (qt6ct + QSS, KDE apps kept on Fusion instead of Breeze), and their file dialogs open
  in yami-files through the portal, like GTK apps and browsers (Tor Browser included).
- **A curtain for streams.** Mod+Shift+H: viewers of a stream, a Discord demo or an OBS recording
  see the frozen screen blurred (or in big pixels, or just the wallpaper), while you keep your
  desktop and a small «hidden» badge. A single shared window is covered too, and notifications
  never reach a recording (settings → services). Settings → screenshots. Needs yami-niri
  (`show-only-in`, `hide-from`); on other niri the windows turn black in the stream instead.
- **Your wallpaper.** Settings → wallpaper: pick an image (or «set as wallpaper» in yami-files and
  yami-view), fill or fit, pixel cells, recolour it into the accent or the palette tones, darken
  it, or let the palette follow the image. The login screen gets the same picture. With
  [skwd-wall](https://github.com/liixini/skwd-wall) its picks (videos and Wallpaper Engine scenes
  too) become the wallpaper and its palette recolours the rice; nothing to configure on its side.
- **Your bar shape.** Full-width strip, a centred panel of any width, or a floating pill with a gap
  and a shadow; the centre modules sit dead centre. Windows can get rounded corners and any border
  width (games stay square).
- **Crisp on any screen.** The shell, yami-files and yami-view render at the monitor's real scale
  (fractional-scale + viewporter) and upscale their pixels by nearest neighbour, so a 4K screen at
  scale 2 shows sharp 2×2 pixels instead of a compositor-smeared image. Tiny5 is drawn on its 8 px
  grid with no antialiasing (16/24/32 px by screen size).
- **Your fonts.** Settings → fonts: any font for the shell, separately for yami-files and
  yami-view, for GTK/Qt apps (readable sans by default, or pixel everywhere) and the terminal.
- **Pixel motion.** Panels and windows dissolve in on a Bayer dither grid, matching niri's custom
  window-open shaders. Settings → look offers four more patterns (scanlines, noise, diagonal,
  blocks), the cell size, an instant close and CRT scanlines over the panels.
- **Light or dark.** One switch (settings → color, or `install.sh --theme light`) turns the whole
  rice light: the shell, GTK/Qt/KDE, kitty, the cursor, the lock and login screens.
- **A launcher that does more.** Type `2+2*3` for an answer, an app or a window title, `vrr` to land
  on that setting, `>cmd` to run a command, `/name` to find a file at home.
- **Your own bar modules.** Any script in `~/.config/yami/modules/` that prints text or JSON
  becomes a module (weather, crypto, GitHub, anything); see [docs/CONFIG.md](docs/CONFIG.md).
- **Small comforts.** Reply to a message right from its notification, a night light (wlsunset),
  the programs of your last session reopened at login (opt-in), themes shared as one file
  (`yami theme export | import`) and the whole setup moved to a new system (`yami export | import`).
- **Bilingual.** English and Russian interface, switchable live in settings.
- **Portable installer.** One script for Arch, Fedora (including atomic), Debian/Ubuntu,
  openSUSE, Void, Alpine, Gentoo and NixOS.

## Screenshots

Real screenshots of a yami session (`tools/readme-shots.sh`).

**yami-files** · thumbnails, places, tabs and split view; the file dialog of every app

<img src=".github/assets/files-en.png" width="100%" alt="yami-files: thumbnails, places, tabs and split view; the file dialog of every app">

**Screenshots** · area, window or screen, then arrows, frames, marker, mosaic and badges

<img src=".github/assets/screenshot-en.png" width="100%" alt="Screenshots: area, window or screen, then arrows, frames, marker, mosaic and badges">

**Settings → colour** · seven palettes or your own accent, the whole desktop follows

<img src=".github/assets/settings-color-en.png" width="100%" alt="Settings → colour: seven palettes or your own accent, the whole desktop follows">

**Settings → keys** · every binding by category, recorded live

<img src=".github/assets/settings-keys-en.png" width="100%" alt="Settings → keys: every binding by category, recorded live">

**yami-view** · real pixels when zoomed, quick edits, set as wallpaper

<img src=".github/assets/view-en.png" width="100%" alt="yami-view: real pixels when zoomed, quick edits, set as wallpaper">

<details>
<summary><b>More screenshots</b></summary>

**Settings → updates** · releases, signatures, rollback

<img src=".github/assets/settings-updates-en.png" width="100%" alt="Settings → updates: releases, signatures, rollback">

**Settings → services** · hand any of them to another program

<img src=".github/assets/settings-agents-en.png" width="100%" alt="Settings → services: hand any of them to another program">

**Sound popup** · outputs, volume, microphone

<img src=".github/assets/popup-sound-en.png" width="560" alt="Sound popup: outputs, volume, microphone">

**The desktop** · bar, widgets and the launcher on the pixel wallpaper

<img src=".github/assets/desktop-en.png" width="100%" alt="The desktop: bar, widgets and the launcher on the pixel wallpaper">

</details>

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
| `--fetch mini\|full` | fastfetch: who, system and desktop (default) or with the hardware too |
| `--no-fork` | keep the packaged niri instead of building yami-niri |
| `--no-tools` | leave out night light, text and QR from screenshots (wlsunset, tesseract, zbar) |
| `--games`, `--no-games` | GameMode and MangoHud (on by default when Steam is installed) |
| `--login`, `--no-login` | SDDM with the yami theme even if another login manager is set / leave it alone |
| `--no-sudo` | home directory only: no packages, no system files |
| `--dry-run` | print the plan, change nothing |
| `--no-count` | do not add this machine to the «installs» badge (one anonymous download, [details](docs/SECURITY.md#install-count)) |
| `--uninstall` | remove the shell, the fork and the dotfiles, restore the backups |
| `--update` | rebuild from this tree keeping every setting, no sudo (what `yami update apply` runs) |
| `--no-anim` | disable the installer animations |

What it does:

1. Detects the distribution from `/etc/os-release` and installs packages with its native package
   manager (niri, portals, polkit, PipeWire, NetworkManager, BlueZ, power-profiles-daemon, Noto
   fonts, cliphist, grim/slurp, kitty, fastfetch, btop, udisks2, ffmpegthumbnailer, bsdtar, mpv (libmpv), gvfs with the MTP/PTP/AFC/SMB backends, qt6ct,
   adw-gtk3, Papirus, SDDM). Names a repository does not carry are reported, not fatal. Enables
   Bluetooth and power profiles, and NetworkManager when nothing else manages the network.
2. Builds the workspace with `cargo build --release --locked` (Rust ≥ 1.89; rustup is used
   when the distribution's toolchain is older) and installs `yami-shell`, `yami-files`,
   `yami-view` and `yami-play` into `~/.local/bin`; registers yami-files for folders and as the portal file
   dialog / FileManager1, yami-view for images, yami-play for video and music, and the Firefox-family portal pref.
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
| `Alt+Shift` | next keyboard layout (settings → keys → layout) |

These are the defaults: change any of them in settings → keys (`yami-shell binds list` prints the
current set).

In the bar: left click on a task focuses, minimizes and restores it, middle click closes, right
click opens the window menu; scroll over workspaces to switch, over the volume to change it. The
track opens the player (drag its position bar to seek), right click pauses, middle click skips.

## The `yami` command

One command for the whole desktop (`~/.local/bin/yami`, the same binary as `yami-shell`);
`yami help` prints it in the palette's colours, `yami help <command>` explains one.

```sh
yami open settings:keys       # launcher clipboard settings[:tab] power lock screenshot cal sound media net bt sys
yami palette ash              # list palettes without an argument; or a colour: yami palette '#3a9fd0'
yami wallpaper ~/x.png        # own wallpaper; no path: the palette's pixel art
yami restart                  # restart the shell (waits while a game has focus)
yami status                   # what runs: shell, portal, niri, the last update check
yami doctor                   # which programs and D-Bus services are there, what is missing and what it costs
yami doctor games             # ntsync, gamemode, MangoHud: what is there, how to add the rest
yami report                   # an archive for an issue: versions, doctor, journal, configs without secrets
yami ocr image.png            # text from an image (like «text» in the screenshot overlay)
yami theme export ~/my.yami   # the look as one file (import: yami theme import FILE)
yami export ~/all.yami        # every setting as one file for a new system (yami import FILE)
yami version                  # every part, the installed commit, the update channel
yami update                   # what is new; apply | list | to X.Y.Z | rollback | status | log | need
```

## Updates

Settings → updates (or `yami update`) checks the repository once a day and shows a card when a
release is out; installing is always your click. The updater keeps its own clone in
`~/.local/share/yami/src`, builds the new version from source with `install.sh --update`
(settings kept, no sudo) and keeps the previous binaries for `yami update rollback`. Channels:
**releases** (`vX.Y.Z` tags) or **every commit** of `main`. The tab shows a small console with the
update log and a progress bar while it runs. An update needs no sudo, so when a new version wants
packages (a new feature's helper) the tab shows them with an **install** button that runs one
`sudo` command in a terminal (`yami update need`, also in `yami doctor`).

Going back is a choice, not a single click: **roll back** opens a list with the previous build
(instant) and the signed releases (`yami update list`, `yami update to 2.1.0`), and only
**install** starts it. Settings an older version does not know are kept aside and come back
when you update again; after any switch the updater makes sure the shell is running.

Releases are signed: once `keys/allowed_signers` holds the author's key, the installed version
accepts only tags signed with it, so a pushed or rewritten tag without the key does not install.
For maintainers:

```sh
tools/release.sh --keygen     # once: an ssh signing key, trusted in keys/allowed_signers (commit it)
tools/release.sh 2.1.0        # CHANGELOG «Unreleased» → 2.1.0 (both languages), versions, check.sh, signed tag, push
```

Updates come from this repository over HTTPS; set `updateRepo` to follow a fork instead.

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

- **Settings tabs**: color, wallpaper, look, fonts, bar (drag module chips and separators; spacing at the
  edges, between modules and around the windows with arrows), widgets,
  dock, windows (gaps between windows and from the screen edges, floating rules, cursor), keys, games, screenshots, screens, system, services
  (notifications, tray, polkit, portals, covers: each can be handed to another program), updates.
- **niri**: the shell owns `yami-binds.kdl`, `yami-fx.kdl` (window rules, blur, games),
  `yami-colors.kdl`, `yami-cursor.kdl` and `yami-outputs.kdl`; `~/.config/niri/user.kdl` is
  included after them and never overwritten, so your binds, rules and outputs there win.
- **IPC**: `yami-shell msg <command>`, where the command is one of `launcher`, `clipboard`,
  `power`, `settings`, `lock`, `bar`, `stacking`, `minimize`, `restore`, `edit`, `reload`,
  `restart`, `quit`, `settings TAB`, `profile` (next power profile), `popup cal|sound|media|net|bt|sys|profile`, `screenshot [area|window|screen]`,
  `wallpaper [PATH]` (no path: the generated pixel wallpaper), `curtain [on|off]` (privacy curtain), `night` (night light on/off).
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
| `shell/play/` | yami-play: video and music player (libmpv) |
| `sddm/yami/` | SDDM login theme (Qt 6 QML, dissolve shader) |
| `config/portal/`, `config/applications/` | portal registration, D-Bus services, desktop entries |
| `config/niri/` | niri configuration (keybindings come from the shell) |
| `config/kitty/`, `config/fontconfig/` | terminal and pixel font rendering |
| `tools/theme.py` | palette export: Qt, KDE, GTK, kitty, fastfetch, niri, wallpaper |
| `tools/apply-theme.sh` | fonts and widget style for Qt and GTK |
| `fonts/` | bundled pixel fonts (SIL OFL) |
| `fork/` | yami-niri: patch series on upstream niri, build script, session wrapper |
| `docs/` | configuration reference, security notes (each also in Russian: `*.ru.md`) |
| `keys/` | release signing keys (`allowed_signers`), read by `yami update` |
| `VERSION`, `CHANGELOG.md` | the release version and what changed in each release (`CHANGELOG.ru.md` in Russian) |
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
| `tools/check.sh` | build without warnings, i18n checks, the English and Russian docs in step (`tools/check-docs.py`), script syntax, niri config, installer dry run (CI runs it too) |
| `tools/map.sh [TEXT]` | every source file with its one-line description |
| `yami-shell shot [TARGET…] --lang en [--transparent] [--frame PNG] -o DIR` | render panels to PNG without a compositor (`--frame`: a real screen under the screenshot overlay); fails on untranslated text |
| `yami-files shot`, `yami-view shot`, `yami-play shot` | the same for the file manager, the viewer and the player |
| `tools/sddm-preview.sh OUT.png` | render the login screen offscreen with your palette (`--window` to try it) |
| `tools/readme-art.py`, `tools/readme-shots.sh` | the README's logo and cards; real screenshots of your session (empty workspace, plain device names, everything put back) |
| `tools/release.sh X.Y.Z` | cut a release: changelog, versions, check, signed tag, push (`--keygen` once for the key) |
| `tools/deploy.sh` | build, install to `~/.local/bin` and restart the running shell (refuses while a game has focus); `--files-only` installs yami-files, yami-view and yami-play only |

## Made with Claude

yami is written with the help of [Claude](https://claude.ai) by Anthropic, through Claude Code:
much of the code, the yami-niri patches and the docs came out of those sessions. Every change is
still built, checked (`tools/check.sh`) and used every day on a real machine before it ships.

## License

Code: MIT, see [LICENSE](LICENSE). Bundled fonts keep their own licenses (SIL Open Font
License, texts in `fonts/`). Monocraft is downloaded from its upstream release at install time
(pinned version, checked against its SHA-256). yami-niri is built from upstream niri (GPL-3.0)
with the patches in `fork/`.
