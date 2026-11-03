# Configuration reference

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
yami open settings:updates                # a panel: launcher clipboard settings[:tab] power lock screenshot cal sound net bt sys
yami palette | yami palette ash | yami palette '#3a9fd0'
yami wallpaper ~/Pictures/x.png           # no path: the palette's pixel wallpaper
yami restart                              # restart the shell (refuses while a game has focus; --force)
yami status                               # shell, portal, niri, last update check
yami doctor                               # programs, D-Bus services, config: what is missing and what it costs
yami version                              # every part with the installed commit
yami update [check|apply|rollback|status|log]
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
| `wallpaper` | `""` | path to your own image (PNG, JPEG, WebP); empty: the generated pixel wallpaper |
| `wallFit` | `"fill"` | `fill`: cover the screen, crop the rest; `fit`: the whole image, margins in the background colour |
| `wallPixel` | `1` | pixel cells of your image: 1 as is, 2…8 → cells of 2, 3, 4, 6, 8, 12, 16 px |
| `wallTone` | `""` | colouring of your image: `""` as is, `tint` the accent's shades, `palette` palette tones with Bayer dithering |
| `wallDim` | `0.0` | darken the wallpaper towards the background colour, 0…0.8 |
| `wallAccent` | `false` | take the accent (and with it the whole palette) from your image; picking a colour by hand turns it off |
| `wallKanji` | `true` | the 闇 glyph in the wallpaper corner |
| `iconTheme` | `""` | icon theme for app icons; empty: auto |
| `iconPixel` | `true` | app icons redrawn as 16×16 pixel art in the palette tones |
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
| `barLeft`, `barCenter`, `barRight` | see `config list` | module ids in order: `launcher` `workspaces` `mode` `tasks` `media` `tray` `net` `netspeed` `bt` `kb` `cpu` `ram` `temp` `vol` `profile` `clock` `notif` `settings` `power` `sep` |
| `cpuStyle` | `"gauge"` | `gauge` `graph` `number` |
| `clockSeconds`, `clockDate` | `false`, `true` | |
| `taskWidth` | `190` | width of a task button, px |
| `barSep` | `"dots"` | separator look: `dots` `line` `blocks` `gap` |
| `barSepW`, `barSepColor` | `10`, `"border"` | separator width; colour `border` `accent` `dim` |
| `barGap` | `8` | gap between modules, px |

### Windows, dock, screenshots, system

| Key | Default | Meaning |
|---|---|---|
| `stacking` | `false` | «desk» mode: every window floats, the shell draws title bars |
| `titlebars` | `true` | title bars in desk mode |
| `floatMedia`, `floatPip` | `true` | photo/video viewers and picture-in-picture open floating |
| `floatApps` | `""` | more app-ids to open floating, comma separated |
| `floatW`, `floatH` | `0.62`, `0.78` | their size, share of the screen |
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
| `mediaArtNet` | `true` | album covers from the player's web links (`curl`, https after redirects, ≤ 16 MB); off: only local files |
| `portalFiles`, `portalCast` | `true`, `true` | file dialogs from yami-files, the screen-share picker from the shell (else GTK / GNOME) |

### Updates

`yami update` and settings → updates work on their own clone, `~/.local/share/yami/src` (your
working copy is never touched). A release is the newest `vX.Y.Z` tag; with `updateChannel`
`main` every commit of the branch is offered. Installing runs `install.sh --update` from the
new tree: it rebuilds from source, keeps every setting and needs no sudo; the previous binaries
go to `~/.local/lib/yami/prev` for `yami update rollback`.

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

Keys are written as niri does: `Mod` (Super), `Ctrl`, `Alt`, `Shift`, then a key name (`Q`,
`Slash`, `Page_Up`, `Print`, `XF86AudioMute`, `WheelScrollDown`). Anything the table cannot
express goes into `~/.config/niri/user.kdl` in plain niri syntax, which wins.

### Widgets

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
| `terminal` | `"kitty"` | «open terminal here» |
| `confirmTrash`, `singleClick`, `split` | `false` | |
| `width`, `height` | `1060`, `660` | window size at start |

Its shortcuts are the usual ones (Ctrl+C/X/V, F2, Delete, Ctrl+T, Ctrl+L, Ctrl+H …) and match by
key position, so they work in any keyboard layout.
