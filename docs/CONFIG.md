# Configuration reference

English · [Русский](CONFIG.ru.md)

Everything the settings window changes is a key in a plain JSON file; editing the file by hand
works the same and is picked up live. A key with a bad value falls back to its default (the rest
of the file is kept), unknown keys are ignored: `yami-shell config check` lists both.

| File | Owner | Edit by hand |
|---|---|---|
| `~/.config/yami-shell.json` | the shell; yami-files, yami-view and `tools/theme.py` read the look from it | yes, live |
| `~/.config/yami-files.json` | yami-files | yes (read at start) |
| `~/.config/niri/config.kdl` | the installer | better not: it is replaced on reinstall |
| `~/.config/niri/user.kdl` | **you** | yes: included last, wins over everything above |
| `~/.config/niri/yami-binds.kdl` | the shell (settings → keys) | no, rewritten |
| `~/.config/niri/yami-fx.kdl` | the shell (windows, look, games) | no, rewritten |
| `~/.config/niri/yami-colors.kdl`, `yami-cursor.kdl` | `tools/theme.py` | no, rewritten |
| `~/.config/niri/yami-outputs.kdl` | the shell (settings → screens) | no, rewritten |
| `~/.config/niri/yami-local.kdl` | the installer (GPU environment) | no |
| `~/.local/state/yami/palette.json` | the shell: the derived palette other apps are coloured from | no |
| `~/.config/xdg-desktop-portal/niri-portals.conf` | the shell (settings → services); kept as is once it no longer starts with `# yami:` | through the settings |
| `~/.local/state/yami/install.json` | the installer and `tools/deploy.sh`: version, commit, source, components | no |
| `~/.local/state/yami/update.json` | the last update check | no |

## The terminal

`yami` is the same binary as `yami-shell` (a link in `~/.local/bin`): without a command it
prints the help instead of starting the shell. `yami help <command>` explains one command.

```sh
yami open settings:updates                # a panel: launcher clipboard settings[:tab] power lock screenshot cal sound media net bt sys
yami palette | yami palette ash | yami palette '#3a9fd0'
yami wallpaper ~/Pictures/x.png           # no path: the palette's pixel wallpaper
yami restart                              # restart the shell (refuses while a game has focus; --force)
yami status                               # shell, portal, niri, last update check
yami doctor                               # programs, D-Bus services, config: what is missing and what it costs
yami doctor games                         # ntsync, gamemode, MangoHud: what is there, one command for the rest
yami report [-o FILE]                     # versions, doctor, this boot's journal, configs in a tar.gz for an issue (secrets scrubbed)
yami ocr FILE                             # text from an image, the same recognition as «text» in the screenshot overlay (needs tesseract)
yami theme export FILE | import FILE      # the look as one file: palette, effects, fonts, bar, widgets, wallpaper (never commands)
yami export FILE | import FILE [--yes]    # every setting as one file; user.kdl, custom keys and bar scripts only with --yes; before an import the current state goes to ~/.local/state/yami/before-import-*.json (undo: yami import that file --yes)
yami clean [--dry-run]                    # remove the build cache (yami-niri sources, the shell's target: gigabytes); --force also drops a tree with your patch work
yami version                              # every part with the installed commit
yami update [check|apply|list|to X.Y.Z|rollback|status|log|need]
yami msg <command>                        # raw shell command (yami help msg)
yami-shell config list                    # every key = value; * marks what differs from the default
yami-shell config get dockPins
yami-shell config set dockSide '"left"'   # VALUE is JSON; a bare word is taken as a string
yami-shell config set animSpeed 0.7
yami-shell config reset dockSide          # one key back to its default
yami-shell config reset                   # the whole file (the old one is kept as .json.bak)
yami-shell config check                   # bad values and unknown keys
yami-shell config kdl                     # what the shell writes to yami-fx.kdl
yami-shell binds list | print | write     # keybindings
```

## yami-shell.json

### Colour and look

| Key | Default | Meaning |
|---|---|---|
| `palette` | `"blood"` | preset: `blood` `ash` `sakura` `amber` `venom` `ice` `dusk` |
| `accent` | `""` | own accent `#rrggbb`; empty: the preset's |
| `tint` | `1.0` | how strongly the graphite takes the accent's hue |
| `glass` | `0.78` | panel opacity (0 glass … 1 solid) |
| `lift` | `0.0` | panel brightness: 0 near-black graphite, 1 noticeably lighter |
| `inner` | `1.0` | opacity of cards, graphs and fields inside panels |
| `dim` | `0.5` | dim behind menus and settings |
| `blurShell`, `blurTerm` | `true` | niri blur under the shell's panels / terminals |
| `animations` | `true` | Bayer dissolve, panel motion; off everywhere when false |
| `animSpeed` | `1.0` | duration multiplier: 0.5 faster, 2 slower |
| `light` | `false` | light theme: paper instead of graphite, the accent deepened until it reads; theme.py switches GTK (adw-gtk3, `prefer-light`), Qt, KDE, kitty, btop, the cursor and the light icon variant with it |
| `reveal` | `"dither"` | how panels appear: `dither` (Bayer 4×4), `scan` (interlaced lines), `noise`, `wipe` (diagonal with a dithered edge), `blocks` (Bayer, double cells) |
| `revealGrain` | `1` | cell size of the reveal, 1–4 |
| `animClose` | `true` | panels close with the reveal backwards; off: they vanish at once |
| `scanlines` | `false` | every second line of the shell's panels a little darker, like an old monitor |
| `wallpaper` | `""` | path to your own image (PNG, JPEG, WebP); empty: the generated pixel wallpaper |
| `wallFit` | `"fill"` | `fill`: cover the screen, crop the rest; `fit`: the whole image, margins in the background colour |
| `wallPixel` | `1` | pixel cells of your image: 1 as is, 2…8 → cells of 2, 3, 4, 6, 8, 12, 16 px |
| `wallTone` | `""` | colouring of your image: `""` as is, `tint` the accent's shades, `palette` palette tones with Bayer dithering |
| `wallDim` | `0.0` | darken the wallpaper towards the background colour, 0…0.8 |
| `wallAccent` | `false` | take the accent (and with it the whole palette) from your image; picking a colour by hand turns it off |
| `wallKanji` | `true` | the 闇 glyph in the wallpaper corner |
| `wallSkwd` | `false` | the wallpaper comes from [skwd-wall](https://github.com/liixini/skwd-wall): the shell reads what it writes (`~/.cache/skwd-wall-v2/last-wallpaper.json`, `colors.json`; `SKWD_WALL_V2_CACHE` and `paths.cache` respected) and changes nothing in its config. On a monitor where skwd-wall draws (pictures by skwd-paper, videos and scenes by skwd-wall-vk, transitions; seen from its niri layer) the shell hides its own wallpaper; otherwise (skwd-wall's pick-only mode) the shell draws the picked image itself in its cells and tones, a video or scene by its preview frame. The lock and login screens show the same picture. `yami-fx.kdl` gets the layer rule that puts skwd-paper's overview backdrop behind the workspaces |
| `wallSkwdColors` | `true` | with `wallSkwd`: the accent is skwd-wall's `primary` and the theme follows its background (light or dark), so a palette made in its theme designer recolours the whole rice |
| `iconTheme` | `""` | icon theme for app icons; empty: auto |
| `iconPixel` | `true` | app icons redrawn as 16×16 pixel art in the palette tones |
| `fetchFull` | `false` | fastfetch with the hardware (kernel, packages, CPU, GPU, memory, disk, display); off: who, OS, wm, shell, terminal, uptime |
| `lang` | `""` | `ru`, `en`, or empty for `$LANG` |

### Fonts

| Key | Default | Meaning |
|---|---|---|
| `fontShell` | `""` | the shell's font (any fontconfig family); empty: Tiny5. Non-pixel fonts are sized to Tiny5's cap height so layouts hold |
| `fontFiles`, `fontView` | `""` | yami-files / yami-view; empty: as the shell |
| `fontUi` | `""` | GTK/Qt apps: empty Tiny5 on its grid, `"sans"` the system sans, or a family (the installer's default is `"sans"`) |
| `fontPt` | `0` | size of a non-pixel `fontUi`, pt; 0: 11 |
| `fontSize` | `0` | size of the pixel `fontUi`: 16, 24, 32 px, or 0 by screen height |
| `fontMono` | `""` | terminal (kitty) and Qt fixed font; empty: Monocraft, else Terminess |
| `browserMenus` | `true` | Firefox-family browsers: right-click menus, dropdowns and tooltips in the palette (nothing else of the browser is touched) |
| `browserFont` | `false` | and the pixel font in the browser's menus and toolbars (never on web pages) |

### Bar

| Key | Default | Meaning |
|---|---|---|
| `barLeft`, `barCenter`, `barRight` | see `config list` | module ids in order: `launcher` `workspaces` `mode` `tasks` `media` `tray` `net` `netspeed` `bt` `kb` `cpu` `ram` `temp` `bat` `vol` `profile` `clock` `notif` `settings` `power` `sep` (`bat`: laptop battery, hidden without one) |
| `cpuStyle` | `"gauge"` | `gauge` `graph` `number` |
| `tempStyle` | `"full"` | the `temp` module: `full` like `cpu` and `ram` (label and `cpuStyle`), `compact` a small thermometer column and the number |
| `trayMode` | `"icons"` | `icons`: every tray icon on the bar; `arrow`: one arrow that opens them as a list (clicks work as on the bar; a dot when an app asks for attention) |
| `clockSeconds`, `clockDate` | `false`, `true` | |
| `taskWidth` | `190` | width of a task button, px (60–400) |
| `barTaskGap` | `4` | gap between task buttons, px (0–16) |
| `barSep` | `"dots"` | separator look: `dots` `line` `blocks` `gap` |
| `barSepW`, `barSepColor` | `10`, `"border"` | separator width, px (2–32); colour `border` `accent` `dim` |
| `barGap` | `8` | gap between modules, px (0–32) |
| `barPadL`, `barPadR` | `6`, `6` | from the left and right edge of the bar to the outermost modules, px (0–64) |
| `barMid` | `12` | from the side modules to the window list (and the centre modules), px (0–64) |
| `barFloat` | `false` | the bar as a separate panel: `barMargin` from the screen edges, a border all round, a hard shadow; windows keep clear of it |
| `barWidthPc` | `100` | bar width, % of the monitor (30–100); below 100 the panel is centred (`barFloat` off: it hangs from the top edge) |
| `barMargin` | `6` | gap between a floating bar and the screen edges, px (0–32) |
| `barCenterAlign` | `"center"` | `barCenter` modules: `center` at the middle of the bar (a long left group only pushes them as far as needed), `after-left` right after the left group. With `tasks` in the middle it stretches either way |

### Windows, dock, screenshots, system

| Key | Default | Meaning |
|---|---|---|
| `stacking` | `false` | «desk» mode: every window floats, the shell draws title bars |
| `titlebars` | `true` | title bars in desk mode |
| `floatMedia`, `floatPip` | `true` | photo/video viewers and picture-in-picture open floating |
| `floatApps` | `""` | more app-ids to open floating, comma separated |
| `floatW`, `floatH` | `0.62`, `0.78` | their size, share of the screen |
| `winGap` | `8` | gap between tiled windows, px (0–64): niri `gaps` in `yami-fx.kdl` |
| `winLeft`, `winRight`, `winTop`, `winBottom` | `8` each | from the screen edge to the windows, px (0–128; the top one from the bar): niri `struts` = edge − gap. Struts in `user.kdl` replace these whole |
| `winRadius` | `0` | window corner radius, px (0–24; 0 square, like pixels): niri `geometry-corner-radius` with `clip-to-geometry`, the border follows it. Games stay square; fullscreen and maximized windows are square anyway; in desk mode with title bars only the bottom corners of floating windows round, so the shell's title bar joins without a gap |
| `winBorder` | `2` | window border width, px (0–8; 0: none); its colours come from the palette |
| `cursor`, `cursorSize` | `"yami"`, `24` | `yami` (pixel, in the palette) or `system` |
| `dock` | `true` | show the dock |
| `dockSide` | `"bottom"` | `bottom` `top` `left` `right` |
| `dockHide` | `"smart"` | `never` `smart` (visible on an empty desk) `always` |
| `dockSize`, `dockAnim` | `40`, `"grow"` | icon size; hover: `grow` `wave` `lift` `none` |
| `dockBounce`, `dockLabels`, `dockRunning`, `dockLauncher`, `dockTrash` | `true` | |
| `dockPins` | | app ids (desktop file names) pinned to the dock |
| `shotDir` | `""` | screenshot folder; empty: Pictures/Screenshots |
| `shotCopy`, `shotSave`, `shotLoupe`, `shotNotify` | `true` | copy, save, magnifier, notification |
| `shotDelay` | `3` | timer, seconds |
| `curtainStyle` | `"blur"` | what stream viewers see while the screen is hidden (Mod+Shift+H), no labels: `blur` the frozen screen blurred, `pixels` in big pixels, `wall` the wallpaper |
| `curtainCell` | `24` | pixel size of `pixels` |
| `lockAfter`, `screenOffAfter` | `10`, `15` | idle minutes; 0: never |
| `sessionRestore` | `false` | at login, start the programs that were open last time and put their first window back on its monitor and workspace (snapshot in `~/.local/state/yami/session.json`; games stay out) |
| `night` | `false` | night light through wlsunset |
| `nightTemp` | `4000` | evening colour temperature, K (2500–6000) |
| `nightGeo` | `""` | `"lat, lon"`: from sunset to sunrise there; empty: by the clock |
| `nightOnH`, `nightOffH` | `20`, `7` | the clock: warm from / until this hour |
| `dnd`, `notifTimeout` | `false`, `7` | do not disturb; seconds a notification stays |

### Games

| Key | Default | Meaning |
|---|---|---|
| `gameFullscreen` | `true` | game windows (Steam/Proton `steam_app_*`, `gamescope`, `gameApps`) open fullscreen |
| `gameVrr` | `true` | VRR while a game is on screen (the output's VRR set to «in games», settings → screens) |
| `gameTearing` | `false` | tearing: frames shown at once, not at vblank (yami-niri only) |
| `gameCache` | `true` | big NVIDIA/Mesa shader caches without cleanup (from the next login) |
| `gameDnd` | `true` | notifications wait in the history while a game or any fullscreen window has focus |
| `gameProfile` | `true` | «performance» power profile while a game has focus, the previous one after |
| `gameApps` | `""` | your games' app-ids, comma separated; `name*` matches the start |
| `gameNtsync` | `true` | `PROTON_USE_NTSYNC=1` for Proton when the kernel has `/dev/ntsync` (from the next login) |
| `gameGamemode` | `true` | the focused game is registered with gamemoded (no `gamemoderun %command%` needed) |

MangoHud gets a config in the palette and the pixel font (`~/.config/MangoHud/MangoHud.conf`) as
long as its first line is yami's; delete that line and theme.py never touches it again.

### Services

What on D-Bus and in the portals yami does itself. Turned off, it is left to another program
(mako or dunst, waybar's tray, polkit-gnome, the GTK/GNOME portals). Notifications, tray and
polkit change after a shell restart (`yami restart`), portals from the next login.

| Key | Default | Meaning |
|---|---|---|
| `agentNotif` | `true` | the notification server (`org.freedesktop.Notifications`) |
| `agentTray` | `true` | the tray (`org.kde.StatusNotifierWatcher`, at most 64 items) |
| `agentPolkit` | `true` | the polkit password window |
| `notifMax`, `notifHistory` | `4`, `50` | cards on screen at once; how many the history keeps |
| `notifImages` | `true` | pictures in cards (icons and previews; at most 1024², files ≤ 16 MB) |
| `notifCast` | `false` | notifications show in recordings and streams; off: viewers never see them (yami-niri ≥ `.5` simply leaves them out, other niri blacks them out) |
| `audioNames` | `{}` | your names for sound devices: PipeWire name (`pactl list short sinks`) → label, e.g. `{"alsa_output.usb-…-00.analog-stereo": "Headphones"}` |
| `mediaArtNet` | `true` | album covers from the player's web links (`curl`, https after redirects, ≤ 16 MB); off: only local files |
| `portalFiles`, `portalCast` | `true`, `true` | file dialogs from yami-files, the screen-share picker from the shell (else GTK / GNOME) |

### Updates

`yami update` and settings → updates work on their own clone, `~/.local/share/yami/src` (your
working copy is never touched). A release is the newest `vX.Y.Z` tag; with `updateChannel`
`main` every commit of the branch is offered. Installing runs `install.sh --update` from the
new tree: it rebuilds from source, keeps every setting and needs no sudo; the previous binaries
go to `~/.local/lib/yami/prev` for `yami update rollback`. `yami update list` shows the last
eight releases with their signatures, `yami update to X.Y.Z` builds and installs one of them (only
what passes `updateVerify`). Before switching to an older version the settings are copied to
`~/.local/state/yami/settings-newer.json`; keys the older version dropped come back from it on the
next switch to a version at least as new. A switch to a whole version also rebuilds yami-niri at
that version's level (from the next login).

An update runs without sudo. When the components chosen at install time (`tools`, `games`, …)
want packages the system lacks — a new version brought a feature with a helper program — the
installer writes them to `~/.local/state/yami/packages-needed`; `yami update need` prints one
`sudo` command (`--run` runs it), `yami doctor` and settings → updates show it too, the tab with an
**install** button that opens it in a terminal. Only package names the repositories have go there.

| Key | Default | Meaning |
|---|---|---|
| `updateCheck`, `updateEvery` | `true`, `24` | check by itself every N hours (only a check and a card: installing is your click) |
| `updateChannel` | `"release"` | `release` (tags) or `main` (every commit) |
| `updateVerify` | `"auto"` | `auto`: once the installed version has keys in `keys/allowed_signers`, only tags signed by them install; `require`: never unsigned; `off`: no check |
| `updateRepo` | `""` | another repository URL; empty: where the install came from |

### Keys

| Key | Default | Meaning |
|---|---|---|
| `binds` | `{}` | binding id → keys, e.g. `{"terminal": "Mod+Return", "launcher-2": ""}` (`""` turns it off). Ids: `yami-shell binds list` |
| `customBinds` | `[]` | your commands: `[{"key": "Mod+Shift+B", "cmd": "kitty -e btop"}]` (run with `sh -c`) |
| `kbLayout` | `""` | keyboard layouts, xkb codes separated by commas (`"us,ru"`, `"de"`); empty: from the system languages (LANG and LC_*: English gives `us`, every other language adds its layout, non-Latin ones after `us`) |
| `kbSwitch` | `"grp:alt_shift_toggle"` | the xkb option that switches layouts (`grp:ctrl_shift_toggle`, `grp:caps_toggle`, `grp:alt_space_toggle`); empty: no switch |

Keys are written as niri does: `Mod` (Super), `Ctrl`, `Alt`, `Shift`, then a key name (`Q`,
`Slash`, `Page_Up`, `Print`, `XF86AudioMute`, `WheelScrollDown`). Anything the table cannot
express goes into `~/.config/niri/user.kdl` in plain niri syntax, which wins.

### Bar modules from scripts

An executable file in `~/.config/yami/modules/<name>` becomes the bar module `x:<name>` (settings
→ bar lists it with the built-in ones). The script prints a line on every update, plain text or
JSON; a script that keeps running streams lines, one that exits is run again after `interval`
seconds (10 by default).

```sh
#!/bin/sh
# ~/.config/yami/modules/weather: a 7×7 icon, the text, a theme colour, a click, every 10 minutes
t=$(curl -fs 'https://wttr.in/?format=%t' || echo '?')
printf '{"text":"%s","icon":"temp","color":"accent","click":"xdg-open https://wttr.in","interval":600}\n' "$t"
```

`color` is a theme role: `accent` `bright` `ok` `warn` `text` `dim`. `icon` is one of the shell's
7×7 icons, e.g. `cpu` `ram` `temp` `net` `wifi` `bt` `bell` `clock` `cal` `music` `play` `power`
`bolt` `leaf` `moon` `sun` `gauge` `chart` `drop` `folder` `image` `camera` `lock` `user` `gear`.

### Widgets

`widgetContrast` (default `true`): widgets without a frame follow the wallpaper under them: on a light
spot they are drawn with the light variant of the theme (dark text, a deeper accent), on a dark one
with the dark variant, and every letter gets a hard one-font-pixel shadow in the opposite tone, so a
clock stays readable on bright or busy wallpapers. Measured on the prepared wallpaper image after a
new wallpaper, a move, or a settings change.

`widgets` is a list of `{"id", "kind", "on", "x", "y", "source", "style", "anim", "size",
"color", "frame", "label", "seconds", "date", "text"}`: `kind` is `clock` `graph` `net` `media`
`cal` `note`; graphs take `source` `cpu` `ram` `temp` `gpu` `swap` and `style` `bars` `line`
`blocks` `wave` `dots`. `x`/`y` of -1 means the default place. Easier to arrange with the
mouse: settings → widgets, edit mode.

## yami-files.json

| Key | Default | Meaning |
|---|---|---|
| `view` | `"grid"` | `grid` or `list` |
| `zoom` | `2` | icon size in the grid, 0…4 |
| `hidden`, `foldersFirst` | `false`, `true` | |
| `sort`, `desc` | `"name"`, `false` | `name` `size` `time` `type` |
| `thumbs`, `thumbMaxMb` | `true`, `40` | thumbnails, and the largest image to thumbnail, MB |
| `sidebar`, `info` | `true`, `false` | places on the left, details on the right |
| `devices` | `true` | drives, USB sticks, phones and cameras in the sidebar |
| `network` | `true` | the «network» section: mounted shares and «connect…» |
| `servers` | `[]` | addresses you connected to (`smb://nas/media`, `sftp://me@host/`), never with a password; one click mounts them again |
| `terminal` | `"kitty"` | «open terminal here» |
| `confirmTrash`, `singleClick`, `split` | `false` | |
| `width`, `height` | `1060`, `660` | window size at start |

Phones, cameras and network shares go through gvfs (`gio mount`): the packages `gvfs-mtp`
(Android), `gvfs-gphoto2` (cameras), `gvfs-afc` (iPhone) and `gvfs-smb` (Windows shares) add the
backends; sftp, ftp and WebDAV are in gvfs itself. A password is asked in the window and handed
to `gio` over a pipe; it is not stored. Kernel mounts (cifs, nfs, sshfs, rclone) show up too.

Its shortcuts are the usual ones (Ctrl+C/X/V, F2, Delete, Ctrl+T, Ctrl+L, Ctrl+H …) and match by
key position, so they work in any keyboard layout.

## yami-play

`yami-play [FILE|DIR|URL…]` plays every format ffmpeg knows through libmpv (the `mpv` package,
loaded at run time). One file opens its folder as a playlist in natural order. The position is
remembered in `~/.local/state/yami/play/` and frames are saved to the Pictures folder.
Keys: space pause, ←/→ 5 s (Shift 1 s, Ctrl 30 s), ↑/↓ or the wheel volume, F fullscreen, M mute,
N/P next/previous, [ ] speed, A audio track, S subtitles, L repeat, X pixel scaling, C save a
frame, `,` `.` frame step, 1…9 jump to 10…90 %, I details, Q quit. `YAMI_PLAY_OPTS="hwdec=no,…"`
passes extra mpv options. The look (palette, language, font) is the viewer's (`fontView`).
