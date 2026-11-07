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
| `~/.config/niri/yami-local.kdl` | the installer (this machine's own niri settings; empty now) | no |
| `/var/lib/yami-greet/look/` | the shell: what the login screen is drawn from (`look.json`: palette, language, monitors, layouts; `wall.png`: the wallpaper), yours since the install | no, rewritten |
| `/var/lib/yami-greet/log/` | the login screen: `greet.log` (its starts, niri's messages), `last` (how the last start went) | no, read with `yami login status` / `log` |
| `~/.local/state/yami/palette.json` | the shell: the derived palette other apps are coloured from | no |
| `~/.config/xdg-desktop-portal/niri-portals.conf` | the shell (settings → services); kept as is once it no longer starts with `# yami:` | through the settings |
| `~/.local/state/yami/install.json` | the installer and `tools/deploy.sh`: version, commit, source, components | no |
| `~/.local/state/yami/update.json` | the last update check | no |

## The terminal

`yami` is the same binary as `yami-shell` (a link in `~/.local/bin`): without a command it
prints the help instead of starting the shell. `yami help <command>` explains one command.

```sh
yami open settings:updates                # a panel: launcher clipboard settings[:tab] power lock walls screenshot cal sound media net bt sys profile tray
yami palette | yami palette ash | yami palette '#3a9fd0' | yami palette wallpaper
yami wallpaper ~/Pictures/x.png           # no path: the palette's pixel wallpaper
yami restart                              # restart the shell (refuses while a game has focus; --force)
yami status                               # shell, portal, niri, last update check
yami doctor                               # programs, D-Bus services, config: what is missing and what it costs
yami doctor games                         # ntsync, gamemode, MangoHud: what is there, one command for the rest
yami report [-o FILE]                     # versions, doctor, this boot's journal, configs in a tar.gz for an issue (secrets scrubbed)
yami ocr FILE                             # text from an image, the same recognition as «text» in the screenshot overlay (needs tesseract)
yami theme export FILE | import FILE|URL  # the look as one file: palette, effects, fonts, bar, widgets, wallpaper (never commands); an imported theme is kept in your themes, a URL must be https
yami theme save [NAME] | list | apply NAME | remove NAME   # your themes in ~/.config/yami/themes (settings → color → themes); save with no name names it after the palette
yami templates [force NAME]                # render your templates and print what came out of each (force overwrites one changed by hand)
yami export FILE | import FILE [--yes]    # every setting as one file; user.kdl, custom keys and bar scripts only with --yes; before an import the current state goes to ~/.local/state/yami/before-import-*.json (undo: yami import that file --yes)
yami login sync                           # give the login screen the shell's look now (the shell does it on every change)
yami login status | log | preview         # the login screen: installed, greetd enabled, its last start; its log; it over the desktop (password yami or Esc)
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
yami-shell config lite-auto               # lite mode on if this machine is weak or virtual (the installer runs it)
yami-shell binds list | print | write     # keybindings
```

## yami-shell.json

### Colour and look

| Key | Default | Meaning |
|---|---|---|
| `palette` | `"blood"` | preset: `blood` `ash` `sakura` `amber` `venom` `ice` `dusk` |
| `accent` | `""` | own accent `#rrggbb`; empty: the preset's |
| `tint` | `1.0` | how strongly the graphite takes the accent's hue |
| `glass` | `0.78` | panel opacity, 0.15 (glass) … 1 (solid); lower values are raised to 0.15, where panels would be all but invisible |
| `lift` | `0.0` | panel brightness: 0 near-black graphite, 1 noticeably lighter |
| `inner` | `1.0` | opacity of cards, graphs and fields inside panels |
| `dim` | `0.5` | dim behind menus and settings |
| `blurShell`, `blurTerm` | `true` | niri blur under the shell's panels / terminals |
| `animations` | `true` | Bayer dissolve, panel motion; off everywhere when false |
| `lite` | `false` | lite mode for weak and virtual machines (old laptops, VMs, software rendering): no blur, no shell or niri animations, opaque inactive windows; the settings above stay as they are and come back when it is off. A fresh install turns it on by itself on a VM, without GPU acceleration, on pre-2011 Intel graphics or with ≤ 4 GB and ≤ 4 threads (`config lite-auto`); `yami doctor` says why |
| `animSpeed` | `1.0` | duration multiplier: 0.5 faster, 2 slower |
| `light` | `false` | light theme: paper instead of graphite, the accent deepened until it reads; theme.py switches GTK (adw-gtk3, `prefer-light`), Qt, KDE, kitty, btop, the cursor and the light icon variant with it |
| `reveal` | `"dither"` | how panels appear: `dither` (Bayer 4×4), `scan` (interlaced lines), `noise`, `wipe` (diagonal with a dithered edge), `blocks` (Bayer, double cells) |
| `revealGrain` | `1` | cell size of the reveal, 1–4 |
| `animClose` | `true` | panels close with the reveal backwards; off: they vanish at once |
| `scanlines` | `false` | every second line of the shell's panels a little darker, like an old monitor |
| `wallpaper` | `""` | path to your own image (PNG, JPEG, WebP); empty: the generated pixel wallpaper |
| `wallDir` | `""` | the folder the wallpaper picker shows as a grid (settings → wallpaper → pick, `yami open walls`; its «folder…» button sets it); empty: `~/Pictures/Wallpapers` (or `wallpapers`), else the folder of the current wallpaper, else `~/Pictures`. Thumbnails share yami-files' cache `~/.cache/yami/thumbs` |
| `wallFit` | `"fill"` | `fill`: cover the screen, crop the rest; `fit`: the whole image, margins in the background colour |
| `wallPixel` | `1` | pixel cells of your image: 1 as is, 2…8 → cells of 2, 3, 4, 6, 8, 12, 16 px |
| `wallTone` | `""` | colouring of your image: `""` as is, `tint` the accent's shades, `palette` palette tones with Bayer dithering |
| `wallDim` | `0.0` | darken the wallpaper towards the background colour, 0…0.8 |
| `wallAccent` | `false` | take the accent from your image, again with every new one; the whole palette follows it (background, panels, borders, the 16 terminal colours, every recoloured program and template). Settings → color → «palette from the wallpaper» or `yami palette wallpaper`; picking a colour by hand turns it off |
| `wallKanji` | `true` | the 闇 glyph in the wallpaper corner |
| `wallSkwd` | `false` | the wallpaper comes from [skwd-wall](https://github.com/liixini/skwd-wall): the shell reads what it writes (`~/.cache/skwd-wall-v2/last-wallpaper.json`, `colors.json`; `SKWD_WALL_V2_CACHE` and `paths.cache` respected) and changes nothing in its config. On a monitor where skwd-wall draws (pictures by skwd-paper, videos and scenes by skwd-wall-vk, transitions; seen from its niri layer) the shell hides its own wallpaper; otherwise (skwd-wall's pick-only mode) the shell draws the picked image itself in its cells and tones, a video or scene by its preview frame. The lock and login screens show the same picture. `yami-fx.kdl` gets the layer rule that puts skwd-paper's overview backdrop behind the workspaces |
| `wallSkwdColors` | `true` | with `wallSkwd`: the accent is skwd-wall's `primary` and the theme follows its background (light or dark), so a palette made in its theme designer recolours the whole rice |
| `iconTheme` | `""` | icon theme for app icons; empty: auto |
| `iconPixel` | `true` | app icons redrawn as 16×16 pixel art in the palette tones |
| `fetchFull` | `false` | fastfetch with the hardware (kernel, packages, CPU, GPU, memory, disk, display); off: who, OS, wm, shell, terminal, uptime |
| `lang` | `""` | `ru`, `en`, `pt-BR`, or empty to follow `LC_ALL`, `LC_MESSAGES`, then `$LANG`; `pt_BR.UTF-8` and `pt` are also accepted |
| `themeApps` | all of them | the other programs theme.py recolours (settings → color → apps, the installer's `--restyle`): `gtk` (gtk.css, settings.ini, gsettings), `qt` (qt6ct, the KDE colour scheme, kdeglobals), `kitty` (its colours and font; off takes the `include yami.conf` line out of `kitty.conf`), `fastfetch` (its `config.jsonc` while that is yami's; your own one is kept and yami's look goes to `yami.jsonc` next to it), `btop`, `firefox` (menus), `mangohud` (only a config yami wrote), `foot` (`foot/yami-colors.ini` and an `include=~/…` line at the end of `foot.ini` under its own `[main]`, so it wins over colours set above; new windows), `cava` (a gradient in the `[color]` section of its config, open cava reloads at once), `starship` (a `[palettes.yami]` table that also retints `red`, `green` and the other names, and `palette = "yami"` unless you chose another palette). A program left out is not touched. Up to 2.6 it was `appThemes` (foot, cava and starship only), read once and carried over |

### Apps in the palette colours

niri's borders, the login screen and your templates always follow the palette; GTK, Qt and KDE,
kitty, fastfetch, btop, Firefox-family menus, MangoHud, foot, cava and starship do when they are
in `themeApps` (all of them unless you untick some). For any other
app write a template: a file in `~/.config/yami/templates` that is the app's colour config with
`{{accent}}`, `{{bg}}`, `{{text}}`, `{{color0}}`…`{{color15}}` and the other palette names
(`.strip` drops the `#`, `.rgb` gives `r, g, b`), plus a line saying where it goes and, if the app
can reload, whom to signal. Every palette change renders it again, and so does any change in the
templates folder (the shell looks every 5 s). It is written only under `~/.config`,
`~/.local/share` or `~/.cache`, never over a file you changed by hand, and not at all while it
names a colour the palette does not have (a typo would break the app's config); files ending in
`.md` are skipped.

Settings → color → apps lists what came out of each template: written, changed by hand (an
«overwrite» button renders it over your edit and keeps that as `*.bak-yami`), an unknown colour, no
`yami-output` line or a target outside those folders. In a terminal, `yami templates` renders them
and prints the same, `yami templates force NAME` overwrites one; the record is
`~/.local/state/yami/restyle.json`.

```ini
# yami-output: ~/.config/mako/config
# yami-reload: USR1 mako
background-color={{bg}}
border-color={{accent}}
text-color={{text}}
```

### Fonts

Settings → look → «pixel look» moves these keys, `iconPixel` and `cursor` together in four steps: smooth (the system sans and cursor everywhere), softer (pixel shell panels only), yami only (GTK and Qt apps smooth, the installer's default) and all pixel.

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
| `barLeft`, `barCenter`, `barRight` | see `config list` | module ids in order: `launcher` `workspaces` `mode` `tasks` `media` `tray` `net` `netspeed` `bt` `kb` `cpu` `ram` `temp` `gpu` `disk` `weather` `bat` `vol` `profile` `clock` `notif` `settings` `power` `sep` (`bat`: laptop battery, hidden without one; `gpu`: graphics load from amdgpu or `nvidia-smi` every 3 s, hidden without either, «sleep» while a laptop's discrete card sleeps; `disk`: how full `/` is) |
| `cpuStyle` | `"gauge"` | `gauge` `graph` `number`, for `cpu` `ram` `gpu` `disk` (`disk` has no history: `graph` draws a gauge) |
| `tempStyle` | `"full"` | the `temp` module: `full` like `cpu` and `ram` (label and `cpuStyle`), `compact` a small thermometer column and the number |
| `trayMode` | `"icons"` | `icons`: every tray icon on the bar; `arrow`: one arrow that opens them as a list (clicks work as on the bar; a dot when an app asks for attention). A network applet (`nm-applet` and the like) is left out while the bar has `net` |
| `trayPixel` | `false` | tray icons (on the bar and in the arrow's list) redrawn in pixel cells in the palette tones, like the dock and the launcher with `iconPixel` (#19) |
| `batHideFull` | `false` | the `bat` module hides while the laptop is on power and not charging: full, or held at a charge limit (#21) |
| `mediaViz` | `"bars"` | the equalizer of the `media` module and the music popup: `bars` `line` `blocks` `wave` `dots`, or `off` (no equalizer, and no audio capture for it) |
| `mediaView` | `"cover"` | what the music popup shows (the button in its header switches it): `cover` (cover, buttons, equalizer), `both` (cover and lyrics), `lyrics`, `minimal` (title and buttons) |
| `clockSeconds`, `clockDate` | `false`, `true` | |
| `taskWidth` | `190` | width of a task button, px (60–400); `tasks` works in any group and is the one module that narrows when the bar runs out of room |
| `barTaskGap` | `4` | gap between task buttons, px (0–16) |
| `barSep` | `"dots"` | separator look: `dots` `line` `blocks` `gap` |
| `barSepW`, `barSepColor` | `10`, `"border"` | separator width, px (2–32); colour `border` `accent` `dim` |
| `barGap` | `8` | gap between modules, px (0–32) |
| `barPadL`, `barPadR` | `6`, `6` | from the left and right edge of the bar to the outermost modules, px (0–64) |
| `barMid` | `12` | from the side modules to the window list (and the centre modules), px (0–64) |
| `barFloat` | `false` | the bar as a separate panel: `barMargin` from the screen edges, a border all round, a hard shadow; windows keep clear of it |
| `barWidthPc` | `100` | bar width, % of the monitor (30–100); below 100 the panel is centred (`barFloat` off: it hangs from the top edge) |
| `barMargin` | `6` | gap between a floating bar and the screen edges, px (0–32) |
| `barOutputs` | `[]` | the screens with a bar, by niri name (`["eDP-1"]`; `niri msg outputs`); empty: every screen. When none of them is connected (a laptop away from its monitor) every screen gets one. Settings → screens → «bar on this screen» |
| `barCenterAlign` | `"center"` | `barCenter` modules: `center` at the middle of the bar (a long left group only pushes them as far as needed), `after-left` right after the left group (the windows of `tasks` sat there before 2.7) |

### Windows, dock, screenshots, system

| Key | Default | Meaning |
|---|---|---|
| `stacking` | `false` | «desk» mode: every window floats, the shell draws title bars |
| `titlebars` | `true` | title bars and the resize corner in desk mode. Desk mode focuses a window on click (and raises it); focus follows the mouse only in tiling |
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
| `shotConfirm` | `space` | `space`: select, then Space or Enter takes the shot; `instant`: the shot is taken as soon as the mouse is released (area, window), and a full-screen shot without the overlay |
| `curtainStyle` | `"blur"` | what stream viewers see while the screen is hidden (Mod+Shift+H), no labels: `blur` the frozen screen blurred, `pixels` in big pixels, `wall` the wallpaper |
| `curtainCell` | `24` | pixel size of `pixels` |
| `lockAfter`, `screenOffAfter` | `10`, `15` | idle minutes; 0: never |
| `sessionRestore` | `false` | at login, start the programs that were open last time and put their first window back on its monitor and workspace (snapshot in `~/.local/state/yami/session.json`; games stay out) |
| `night` | `false` | night light through wlsunset |
| `nightTemp` | `4000` | evening colour temperature, K (2500–6000) |
| `nightGeo` | `""` | `"lat, lon"`: from sunset to sunrise there; empty: by the clock |
| `nightOnH`, `nightOffH` | `20`, `7` | the clock: warm from / until this hour |
| `dnd`, `notifTimeout` | `false`, `7` | do not disturb; seconds a notification stays |

### Login screen

Settings → system → login screen. The shell publishes these keys with its palette, language and
monitors to `/var/lib/yami-greet/look`; the login screen reads them at its next start.

| Key | Default | Meaning |
|---|---|---|
| `greetWall` | `"desktop"` | its wallpaper: `desktop` the same picture as the desktop, `pixel` the palette's pixel wallpaper, `plain` none (the theme colour) |
| `greetDim` | `0.55` | the veil over the wallpaper (0.2–0.9) |
| `greetClock`, `greetHost` | `true` | the big clock and date; the machine name in the corner |
| `greetUsers` | `"list"` | `list`: the users to pick from with arrows (and «another user»); `name`: no list, the name is typed |
| `greetPower` | `true` | sleep, reboot and power off buttons (where logind or elogind allows them) |
| `greetOff` | `2` | minutes before the screens go off; 0: never |
| `greetNumlock` | `true` | numlock on |

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
| `automount` | `true` | drives, memory cards, discs, phones (Android file transfer), cameras and iPhones mount by themselves when they are plugged in, like in GNOME and KDE: what the volume itself asks to be mounted (removable media yes, partitions of internal disks no), once per plug, so what you unmount stays unmounted. The shell listens to udev (a netlink socket filtered in the kernel to `block` and `usb`, no polling) and asks `gio`. Encrypted drives are not opened by themselves: their card opens yami-files' password window. Nothing mounts while the screen is locked: it waits for the unlock. Settings → services |
| `automountOpen` | `false` | open what was mounted in yami-files at once; off: a card with «open» |
| `notifMax`, `notifHistory` | `4`, `50` | cards on screen at once; how many the history keeps |
| `notifPos` | `"top-right"` | where the cards appear: `top-right`, `top-left`, `top`, `bottom-right`, `bottom-left`, `bottom` (settings → notifications); they keep clear of the bar and the dock |
| `osdPos` | `"bottom"` | the volume and brightness OSD: `bottom`, `top` (under the bar) or `center` |
| `notifOffset`, `osdOffset` | `10`, `64` | distance of the notification cards and of the OSD from their screen edge, px (0–400); next to the bar it is counted from the bar |
| `notifImages` | `true` | pictures in cards (icons and previews; at most 1024², files ≤ 16 MB) |
| `notifCast` | `false` | notifications show in recordings and streams; off: viewers never see them (yami-niri ≥ `.5` simply leaves them out, other niri blacks them out) |
| `audioNames` | `{}` | your names for sound devices (outputs and microphones): PipeWire name (`pactl list short sinks`, `pactl list short sources`) → label, e.g. `{"alsa_output.usb-…-00.analog-stereo": "Headphones"}` |
| `mediaArtNet` | `true` | album covers from the player's web links (`curl`, https after redirects, ≤ 16 MB); off: only local files |
| `lyricsNet` | `true` | lyrics from lrclib.net (artist, title and length are sent) when the player has none and there is no `.lrc` or `.txt` file next to the track; asked only while the music popup shows lyrics, answers cached in `~/.cache/yami/lyrics` |
| `weatherCity`, `weatherUnits` | `""`, `"c"` | the `weather` bar module and widget: a city (`Berlin`) or `latitude, longitude`, asked from open-meteo.com every 20 minutes while one of them is shown; `f` gives °F and mph. Settings → services → network |
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
seconds (10 by default). The same script can also be a desktop widget (settings → widgets →
script), where `\n` in the JSON `text` starts a new line; the bar shows the first line.

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
`cal` `weather` `script` `note`; graphs take `source` `cpu` `ram` `temp` `gpu` `swap` and
`style` `bars` `line` `blocks` `wave` `dots`; a `script` widget takes the script's file name as
`source`. `x`/`y` of -1 means the default place. Easier to arrange with the
mouse: settings → widgets, edit mode.

## yami-files.json

| Key | Default | Meaning |
|---|---|---|
| `view` | `"grid"` | `grid` or `list` |
| `zoom` | `2` | icon size in the grid, 0…4 |
| `hidden`, `foldersFirst` | `false`, `true` | |
| `sort`, `desc` | `"name"`, `false` | `name` `size` `time` `type` |
| `thumbs`, `thumbMaxMb` | `true`, `40` | thumbnails, and the largest image to thumbnail, MB. The cache `~/.cache/yami/thumbs` (shared with the wallpaper picker) tidies itself once a day: thumbnails not opened for 120 days go, and it stays under 512 MB |
| `sidebar`, `info` | `true`, `false` | places on the left, details on the right |
| `devices` | `true` | drives, USB sticks, phones and cameras in the sidebar |
| `network` | `true` | the «network» section: mounted shares and «connect…» |
| `servers` | `[]` | addresses you connected to (`smb://nas/media`, `sftp://me@host/`), never with a password; one click mounts them again |
| `placesOrder` | `[]` | your order of the places: `home`, `desktop`, `documents`, `downloads`, `music`, `pictures`, `videos`, `trash` and bookmark paths; drag a place along the sidebar to set it |
| `placesHidden`, `devsHidden` | `[]`, `[]` | places and devices you hid (right click → hide; right click → show hidden brings them back) |
| `collapsed` | `[]` | folded sidebar sections: `places`, `devices`, `network` (a click on the section title) |
| `terminal` | `"kitty"` | «open terminal here» |
| `confirmTrash`, `singleClick`, `split` | `false` | |
| `width`, `height` | `1060`, `660` | window size at start |

Phones, cameras and network shares go through gvfs (`gio mount`): the packages `gvfs-mtp`
(Android), `gvfs-gphoto2` (cameras), `gvfs-afc` (iPhone) and `gvfs-smb` (Windows shares) add the
backends; sftp, ftp and WebDAV are in gvfs itself. A password is asked in the window and handed
to `gio` over a pipe; it is not stored. Kernel mounts (cifs, nfs, sshfs, rclone) show up too.
A phone shows in the sidebar as soon as file transfer is on (the shell's `automount` mounts it by
itself). An encrypted drive (LUKS, BitLocker) asks for its password on a click (`yami-files
--unlock /dev/…` opens that window); udisks opens it, and «eject» locks it again before the
drive is powered off. A right click on a place or a drive opens it in a new tab or the other pane.

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
