# Changelog

What changed in each release, newest first. `yami update` shows the section of the release it
offers; `tools/release.sh X.Y.Z` turns «Unreleased» into the release (or writes it from the commit
subjects when that section is empty).

## Unreleased

- Light theme: settings → color → theme (or `install.sh --theme light`). The same palette on light paper with a deeper accent that stays readable; GTK (adw-gtk3 and `prefer-light`), Qt, KDE, kitty, btop, Firefox menus, the cursor, the lock and login screens and the light/dark icon variant (Papirus, Gruvbox-Plus) follow it
- Effects in settings → look: five reveal patterns (dither, scanlines, noise, diagonal, blocks), the cell size, the closing animation on or off, CRT scanlines over shell panels. The default dither is 1.7× faster than before (0.6 ms for a full 1080p frame)
- Launcher: a calculator (`2+2*3`, `=sqrt(2)`, Enter copies), open windows and settings places in the results (`vrr` → screens), `>` runs a command in a terminal, `/` finds files in your home folder, `?` lists settings; Ctrl+A selects the search, Ctrl+C copies it. Typed text is no longer passed through the interface translation
- Games: ntsync for Proton (`PROTON_USE_NTSYNC=1` when `/dev/ntsync` exists), the focused game registered with GameMode; `yami doctor games` shows what the system has and one install command for the rest. The games tab is grouped by what each switch touches: game windows, while you play, Proton and the driver
- MangoHud in the palette and the pixel font (only when the config is not your own)
- Bar modules from scripts: an executable in `~/.config/yami/modules/` prints text or JSON (`text icon color click interval`) and becomes a bar module
- Night light through wlsunset (by coordinates or by the clock) with a `night` bar module
- Reply from a notification for messengers that offer it (`inline-reply`)
- Session restore (off by default): the programs that were open come back on their workspaces at the next login
- Screenshots: a colour picker tool (hex to the clipboard), text recognition (tesseract) and QR codes (zbarimg) from the selection
- `yami theme export | import FILE` shares a look as one file (never commands or scripts); `yami export | import FILE` moves every setting to a new system (commands only with `--yes`)
- `yami report` packs versions, doctor, this boot's journal and configs for an issue, with passwords, tokens, the home path, user and host scrubbed
- `yami doctor` mentions btrfs/zfs snapshots (snapper, timeshift) before big updates
- CI: every distro family the installer supports (Arch, Fedora, Debian, Ubuntu, openSUSE, Void, Alpine) checks the package names, builds the workspace and runs the installer's dry run; `install.sh --print-packages` prints the lists
- Installer: two new components with flags: `tools` (night light, text and QR from screenshots: wlsunset, tesseract, zbar; `--no-tools`) and `games` (GameMode, MangoHud; on when Steam is installed, `--games`/`--no-games`). An update keeps the choice and turns a newly added component on at its default; since updates run without sudo, the packages a new version wants are listed in settings → updates with an **install** button, in `yami doctor` and `yami update need [--run]`
- Settings: tabs sorted by meaning (screens next to windows), desk mode and title bars moved to windows, the cursor to look, the terminal font and fastfetch to fonts; the night light says what it needs and hides the hours when it follows the sun; a long value no longer runs over its label
- Security: `yami import` writes only the files `yami export` makes (a crafted file could drop anything into `~/.config`, autostart included); reassigned keys and the file manager's terminal come in only with `--yes`; key names, font names, app-ids and bar script names from the settings file are checked before they reach niri, kitty, GTK or Qt configs (a font name with a line break could add a command to kitty.conf through `yami theme import`); `yami report` and screen text recognition use a private 0700 folder instead of a predictable one in /tmp, and the report hides coordinates, servers and key commands
- Code: app.rs, the file manager's fm.rs, draw.rs and app.rs, settings.rs, snap.rs and update.rs split into smaller modules; Firefox menus got their shadow back (the CSS colour was written in the wrong order and was invisible)

## 2.4.3

- Disk: after a successful yami-niri build the installer removes its sources and build cache (~4 GB; the installed binary does not need them, the next build fetches them again). `--keep-build` or `YAMI_KEEP_BUILD=1` keeps them; a tree with your own edits or commits is never touched
- `yami clean [--dry-run]` removes the build cache by hand (yami-niri sources, the shell's target) and `yami doctor` shows how big it is
- `install.sh --no-monocraft` skips the 31 MB Monocraft font (the terminal uses Terminess; updates remember it, `--monocraft` brings it back)

## 2.4.2

- Battery: a pixel icon on the 2 px grid like the other bar icons (notched case, six charge columns), «on AC power» and time left in the power popup, a notification at 10% and 5%
- Wi-Fi: a saved network without its password (from the live session's KWallet, a changed password) now opens the password field instead of a dead «password required»; a wrong password asks again; passwords go to NetworkManager over D-Bus, not on the nmcli command line; WPA3-only and WEP networks; 802.1X networks point to «all connections»; works on a Russian-locale system
- Bluetooth: new devices are paired and trusted before connecting, errors show in the popup, «device manager» opens blueman (or bluetoothctl) instead of the process monitor; «all connections» falls back to nmtui when nm-connection-editor is missing
- The «installs» badge now counts installs: the installer downloads one empty release file once per machine (nothing is sent; `--no-count` or `YAMI_NO_COUNT=1` to skip). The old badge was GitHub's clone traffic, bots included

## 2.4.1

- Hybrid laptops: the GPU sensor no longer wakes a sleeping NVIDIA card (it shows «sleep» instead of running nvidia-smi every 3 s), and the installer leaves the NVIDIA driver out of the initramfs there

## 2.4.0

- Window gaps in settings → windows: between windows and from each screen edge (the top one from the bar), with arrows or the wheel; the shell writes niri's `gaps` and `struts` (`winGap`, `winLeft`, `winRight`, `winTop`, `winBottom`)
- Bar spacing in settings → bar: the left and right edge, the gap between modules, the separator width, the space around the window list and between window buttons, the window button width. Arrows or the wheel change them (`barPadL`, `barPadR`, `barGap`, `barSepW`, `barMid`, `barTaskGap`, `taskWidth`)
- A battery module for laptops (`bat`: charge, charging, low warning; hidden without a battery), on by default for new installs
- AMD graphics: temperature and load for the `gpu` graph come from the kernel, not only from nvidia-smi
- The lock screen also covers a monitor plugged in while locked; users from LDAP, SSSD or systemd-homed can unlock (the system login service checks them)
- Updates: a release signed by a key outside `keys/allowed_signers` is refused on every git version
- The OFL licence texts of DotGothic16 and Terminess are in `fonts/`; `tools/release.sh` sets the version of every crate (yami-play stayed at 2.2.1)

## 2.3.0

- yami-play: a video and music player in the shell's pixels. Every format ffmpeg knows (libmpv, loaded at run time), hardware decoding, subtitles and audio tracks, a cell timeline, the folder as a playlist, resume where you stopped, speed, frame step, saving a frame and a pixel scaling mode; the screen stays on while a video plays. Video and music open in it after the installer runs
- yami-files: phones and cameras show up under devices (Android over MTP, cameras over PTP, iPhone), with a hint when the gvfs backend is missing
- yami-files: a «network» section: mounted shares, «connect…» for smb, sftp, ftp, WebDAV and nfs with a password window, addresses remembered without passwords; kernel mounts (cifs, nfs, sshfs) are listed and can be unmounted
- The installer adds mpv (libmpv) and the gvfs backends for phones, cameras and Windows shares
- The installer no longer hangs: a closed terminal leaves the menus, arrow keys work over slow SSH and on the Linux console, sudo never waits for a password hidden behind the progress bar, git never asks for a login
- Pasting into a text field no longer freezes the window when the clipboard owner hangs; a hung shell no longer blocks `yami` commands and updates

## 2.2.1

- fastfetch is short by default: who, OS, window manager, shell, terminal and uptime, no hardware. The installer asks (`--fetch mini|full`), settings → look switches it
- Roll back without surprises: the button opens a choice of the previous build (instant) or any signed release, and only «install» starts it; `yami update list` and `yami update to X.Y.Z` do the same in a terminal
- Settings a rolled-back version does not know are kept aside and come back with the next update
- After any update or rollback the updater checks that the shell answers and starts it if not, even when the installed version's own installer did not
- After an update the shell comes back by itself: the restart no longer took the updater for a running shell and left the desktop without a bar
- Settings → updates shows a small console with the update log, a progress bar by installer step and the release notes before you install

## 2.2.0

- `audioNames`: your own names for sound devices in the sound popup
- yami-files: `devices` hides drives and USB sticks from the sidebar
- Recordings and streams no longer show black boxes where notifications and the curtain's «hidden» badge are: on yami-niri 26.04-yami.5 they are simply left out (`hide-from`). Settings → services → notifications in recordings shows them instead
- The privacy curtain also covers a single window shared on its own (OBS window capture, Discord window share), on yami-niri 26.04-yami.5
- The launcher's search hint no longer starts under the text cursor
- yami-files scrolls with the wheel at the screen's frame rate instead of in steps
- The notification countdown bar moves smoothly
- Keyboard layouts are a setting (settings → keys → layout): by default they follow the system languages, so an English system gets `us` alone and a Russian one `us,ru`; the switch combination is a choice too (`kbLayout`, `kbSwitch`)
- Settings → widgets names graph widgets in English in the English interface

## 2.1.0

- `yami`: one command for the whole desktop (open, palette, wallpaper, restart, status, doctor, update)
- updates from settings or `yami update`: releases or every commit, signed tags, rollback
- releases are signed: from this version on, `yami update` installs only tags signed with the key in `keys/allowed_signers`
- settings → services: the notification server, tray, polkit window, portals and covers can be turned off
- notification and tray pictures are size-checked; covers and icon paths are read with a cap
- the album-cover cache no longer grows with every track
- the project lives at github.com/Trendorin/yami: the installer and updates fetch from there
- `yami-shell shot --transparent`: panels with real alpha, as the README pictures use

## 2.0.0

- the yami desktop: shell, yami-files, yami-view, yami-niri, SDDM theme, multi-distro installer
