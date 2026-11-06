# Changelog

English · [Русский](CHANGELOG.ru.md)

What changed in each release, newest first. `yami update` shows the section of the release it
offers (from CHANGELOG.ru.md when the interface is Russian); `tools/release.sh X.Y.Z` turns
«Unreleased» into the release in both files (and refuses while either is empty). Every line here
has its Russian line in CHANGELOG.ru.md: `tools/check.sh` checks that.

## Unreleased

- Fix: a fresh install got a shorter bar: the settings file the installer writes was taken for the pre-2.0 format, so the right side lost the battery, the power profile and the separators, and a `barRight` written by hand into such a file was dropped. Now only a file with the old keys is converted
- Fix: skwd-wall videos and Wallpaper Engine scenes (#3): yami now sees that skwd-wall draws them (its `skwd-wall-vk` layer) and no longer draws its own picture on that screen, which after a shell restart could cover the live wallpaper
- Docs in Russian: CHANGELOG, docs/CONFIG, docs/SECURITY and fork/README have Russian twins (`*.ru.md`), linked at the top of each file and from README.ru.md (which also gets back its missing «building the shell by hand» part); `tools/check.sh` keeps every pair in step (the same structure, code, links and anchors), so neither falls behind. With a Russian interface, settings → updates shows the release notes in Russian
- Media and sound are two popups (#10): the `media` module opens the player alone (cover, buttons, the equalizer, time; drag the position bar to seek, also in the media widget), `vol` the sound settings alone, with a choice of the microphone as well as the output when there are several; without a `media` module or a media widget (as on the default bar) the player stays at the bottom of the sound popup. Right click on the track pauses or plays, middle click skips to the next one; `yami open media` opens it from a key or the terminal
- Wallpaper picker (#11): settings → wallpaper → «pick an image…» opens a grid of thumbnails of your wallpaper folder (`~/Pictures/Wallpapers` by default, `wallDir`, «folder…» to change it), the palette's pixel wallpaper first; a click sets it and goes back to the settings, arrows and Enter work too. Thumbnails are made in the background only for the rows on screen and share yami-files' cache. «another file…» keeps the old file dialog; `yami open walls` (or a key you bind in settings → keys) opens the picker on its own
- Tray behind an arrow (#11): settings → bar → look → tray: `arrow` (`trayMode`) shows one arrow on the bar instead of every icon; it opens the tray apps as a list with their names, where clicks work as on the bar and an app's menu opens under its row. A dot on the arrow when an app asks for attention
- Compact temperature (#11): settings → bar → look → temperature: `compact` (`tempStyle`) draws the `temp` module as a small thermometer column and the number, about a third of its usual width
- Less work while idle: a panel whose new frame is the same as the last one is not sent to niri again (the bar redraws every second, its picture changes about once a minute), the sound state asks PipeWire only about what changed, the equalizer thread sleeps while nobody sees it and computes the spectrum once per frame, the settings file is compared only when the settings changed, font files are mapped once per process, and theme.py runs at a lower priority.
- Desk mode behaves like a desktop: a window gets focus on click and comes to the front with it (focus follows the mouse only in tiling now), so a title bar of a window behind no longer shows over the window in front, and every floating window has a resize corner at the bottom right (with niri's `prefer-no-csd` most apps had no edge to drag).
- Lite mode for weak and virtual machines (from a review: «extremely slow in a VM», a 15-year-old laptop locking up at login). Settings → look → «lite mode» (`lite`) turns off blur, the shell's and niri's animations and the translucency of inactive windows in one switch and leaves your settings as they were. A fresh install turns it on by itself in a VM, without GPU acceleration, on pre-2011 Intel graphics or with ≤ 4 GB of memory and ≤ 4 threads; `yami doctor` tells why it suggests it.
- One «pixel look» slider (asked in the same review: «a slider to set my pixelation lower»). Settings → look: smooth, softer, yami only, all pixel: the fonts of the shell, the apps and GTK/Qt, pixel icons and the cursor move together; the separate keys stay for fine tuning.
- The installer asks for the administrator password once: every root step goes through one root shell opened at the start (sudo or doas), with no credential cache left for build scripts. Packages missing from the distro's repositories are dropped from the batch instead of installing the rest one by one (on Alpine with doas that meant a password prompt per package). On a machine with less than 6 GB of memory or 4 threads the niri fork is off by default and cargo builds with as many jobs as the memory allows.
- yami-files looks for new drives, phones and network mounts only when the system's list of them changes (it used to start `lsblk` and `gio` every 4 seconds while a window was open), and an idle window ticks once a second; the D-Bus helper threads of the shell and yami-files no longer wake twice a second forever (zbus's `blocking` pool, held at 1.6).
- Caches tidy themselves: the thumbnail cache shared by yami-files and the wallpaper picker drops thumbnails not opened for 120 days and stays under 512 MB, and yami-play forgets resume positions of files not played for a year.
- Notification and OSD position (#8): settings → system → notifications: the cards go to any corner or the middle of the top or bottom edge (`notifPos`), the volume and brightness OSD to the bottom, the top or the centre (`osdPos`); cards keep clear of the bar and the dock.
- yami-view and yami-play tick once a second while nothing on screen is about to change (four times a second only while their panels are about to hide, faster only while something moves).
- Fix: `glass` set below 0.15 by hand (the slider stops there) made panels all but invisible, so their shadow and blur looked like broken frames (#8); lower values are raised to 0.15.
- The settings window moves by its header (#8): drag it anywhere on the screen; it opens in the middle again next time.
- Fix: Mod+F did nothing in desk mode (#8): it was `maximize-column`, which leaves floating windows alone; on the desk it now maximizes the window to the screen edges, and in tiling it still maximizes the column.
- On a weak machine (less than 6 GB of memory or 4 threads) the installer and `yami update` build the apps with thin LTO: the compiler's peak memory is about half (1.3 GB instead of 2.4 GB with two jobs), so a 4 GB laptop no longer swaps through the build.
- Fix: after an update OBS and other recorders could find no screen to capture until the next login: the update stopped the screen-sharing and file-dialog backend (`yami-files --portal`) and nothing started it again in time. Now `yami-files --portal-reload` hands over to the new binary in the same process (the portal keeps talking to it), waits while a recording or a file dialog is open, and starts the backend when none runs; the installer, `yami update` and `tools/deploy.sh` use it.
- The installer puts each program into place in one step (a running copy never sees a half-written file) and stops when the apps fail to build, on a first install and on `yami update` alike: it used to go on writing configs for programs that were not there, and to build the compositor fork anyway.
- Fix: `fork/yami-niri.sh off` left the fork's keys (`allow-tearing`, `show-only-in`, `hide-from`) in `yami-fx.kdl`, so at the next login the packaged niri refused the config and started with its own, without the shell. `off` now strips them the way `session.sh` does, and the shell writes them only while the next login runs the fork too.
- Less memory: each yami program gives the kernel back the pages of its own code it has stopped running (start-up, the settings window half a minute after it closes, a finished recording or file dialog in the portal backend). The shell idles at about 12 MB instead of 14.5, the portal backend at under 1 MB instead of 5.5, a file manager window at 19 MB instead of 23.
- yami-view keeps one copy of a big picture while it fits the window (the full-size copy for zooming in is made when you zoom), drops a picture that finished decoding after you moved on, and holds only the current picture and its neighbours: a 4K picture takes 107 MB instead of 140.
- yami-files keeps at most 32 MB of thumbnails in memory: a folder with thousands of pictures no longer grows the window by hundreds of megabytes; the thumbnails on screen stay, the oldest of the rest are dropped and come back from the disk cache when you scroll to them.
- yami-play at a fractional scale no longer clears and redraws its controls layer for every video frame while the controls are hidden.
- The shell measures its own memory (settings → about) only while the settings window is open, as PSS like `yami status`, and the address in the network module comes from the kernel instead of a run of `ip` on the drawing thread.
- yami-files copies through the kernel (`copy_file_range`): on btrfs and xfs a copy inside one disk is a reflink, so a 4 GB video copies at once and takes no space until one of the two is changed; between disks the data no longer passes through the program. Phones, network shares and other FUSE mounts still copy the old way.
- Screenshots without the confirm step (#13): settings → screenshots → «take the shot»: `instant` (`shotConfirm`) takes the shot as soon as the mouse is released after selecting an area or clicking a window, and a full-screen shot (Ctrl+Print) at once, without the overlay; copy and save follow the same settings. Fix: a second Space or Enter (or a held key repeating) while the shot was being taken made a second copy, file and notification.
- The music popup has four views (#13), switched by the button in its header or in settings → bar → look (`mediaView`): the cover with the equalizer, the cover with lyrics, lyrics alone and a short one (title and buttons). Lyrics come from the player, from a `.lrc` or `.txt` next to the track, or from lrclib.net (settings → services, `lyricsNet`; asked only while the popup shows lyrics, cached); with timestamps the sung line is lit and kept in view. No lyrics: the popup shows the cover. The equalizer of the popup and of the `media` module can be turned off or given another style (`mediaViz`); off, nothing captures audio for it.
- Notification and OSD distance from the edge (#8): settings → system → notifications → «from the edge» under each position (`notifOffset`, `osdOffset`); next to the bar it is counted from the bar.
- Bar modules `gpu` and `disk` (#8): graphics load (amdgpu, or `nvidia-smi` every 3 s; a sleeping laptop card is not woken) and how full `/` is, in the `cpuStyle` of `cpu` and `ram`; settings → bar → add.

## 2.5.0

- Floating, partial-width bar (#5, «Bar: floating (partial-width) bar option + geometric centering for barCenter modules»). Settings → bar → look: `barFloat` puts the bar on its own panel with a gap from the screen edges, a border all round and a hard pixel shadow; `barWidthPc` (30–100 %) makes it narrower and centred, docked to the top edge or floating; `barMargin` sets the gap. A live preview shows the shape. Popups, tray menus, a top dock, widgets, the stream badge and desk-mode title bars follow the bar, and windows keep clear of it
- Centred middle modules (#5). `barCenterAlign`: `center` (the new default) puts the `barCenter` modules at the geometric middle of the bar and shifts them only as far as a long left group (a track title in `media`) requires; `after-left` keeps the old placement. A middle that holds the window list still stretches
- skwd-wall support (#3, «skwd wall support»). Settings → wallpaper, shown when skwd-wall is installed: its picks become the wallpaper. Where skwd-paper draws (videos, Wallpaper Engine scenes, transitions) the shell steps aside on that monitor; in skwd-wall's pick-only mode the shell draws the picked image itself, in its cells and tones, a video or scene by its preview frame. With `wallSkwdColors` its palette, custom ones from its theme designer included, sets the accent and light or dark mode for the whole rice. The lock and login screens show the same picture. Everything is read from the files skwd-wall writes; its configuration is never touched
- Widgets readable on any wallpaper (#7, «Add auto contrast checker against wallpaper and switch color between white and black»). Widgets without a frame measure the wallpaper under them: on a light spot they use the light variant of the theme (dark text, a deeper accent), on a dark one the dark variant, and every letter gets a hard one-font-pixel shadow in the opposite tone. Measured again after a new wallpaper, a move or a settings change; `widgetContrast`, settings → widgets
- Bar modules from scripts, the scriptable part of #6 («Feature parity gaps vs Noctalia v5: weather module and scriptable custom widgets»). An executable in `~/.config/yami/modules/` prints text or JSON (`text icon color click interval`) and becomes a bar module; docs/CONFIG.md has a weather script as the example. A built-in weather module and script-fed desktop widgets are still open
- Night light, requested in #8 («OSD/notification positioning, night-light, AUR package, and templates»): wlsunset from sunset to sunrise at your coordinates, or by the clock, with a `night` bar module and `yami msg night`
- Themes as one file, the templates part of #8: `yami theme export | import FILE` shares a look (palette, effects, fonts, bar, widgets, wallpaper; never commands or scripts), `yami export | import FILE` moves every setting to a new system
- Bar popups open below the bar, reported in #8: the calendar with the notification history, sound, network and the other popups covered the lower half of the bar since 2.0; they now open just below it, wherever the bar is
- The wallpaper picker comes to the front, reported in #8: «choose an image…» opened the file dialog under the settings panel, where it could not be used. Settings step aside while you pick and come back on the wallpaper tab
- Window corners and border, settings → windows: `winRadius` rounds windows through niri (the border follows; games stay square, fullscreen and maximized windows are square anyway, and in desk mode only the bottom corners round so the shell's title bar joins without a gap); `winBorder` sets the border width or turns it off
- Text from screenshots, much better. The selection is prepared the way tesseract reads best (dark text on light, enlarged when the text is small, the shell's pixel font kept at its own size, more contrast); several layouts are tried at once and the most confident wins; English and Russian are read separately and chosen per line, so `error` no longer comes out as Cyrillic `еггог` and Russian words do not turn into Latin look-alikes; code and terminal text keep their indentation. On screen-like samples (dark and light themes, 11–16 px, noise, gradients, terminals, mixed languages) the character error rate fell from 4.9 % to 2.1 %. `yami ocr FILE` does the same for any image
- Screenshot tools: a colour picker (hex to the clipboard), text recognition (tesseract) and QR codes (zbarimg) from the selection
- Light theme: settings → color → theme (or `install.sh --theme light`). The same palette on light paper with a deeper accent that stays readable; GTK (adw-gtk3 and `prefer-light`), Qt, KDE, kitty, btop, Firefox menus, the cursor, the lock and login screens and the light/dark icon variant (Papirus, Gruvbox-Plus) follow it
- Effects, settings → look: five reveal patterns (dither, scanlines, noise, diagonal, blocks), the cell size, the closing animation on or off, CRT scanlines over shell panels. The default dither is 1.7× faster than before (0.6 ms for a full 1080p frame)
- Launcher: a calculator (`2+2*3`, `=sqrt(2)`, Enter copies), open windows and settings places in the results (`vrr` → screens), `>` runs a command in a terminal, `/` finds files in your home folder, `?` lists settings; Ctrl+A selects the search, Ctrl+C copies it. Typed text is no longer passed through the interface translation
- Games: ntsync for Proton (`PROTON_USE_NTSYNC=1` when `/dev/ntsync` exists) and the focused game registered with GameMode; `yami doctor games` shows what the system has and one install command for the rest; MangoHud in the palette and the pixel font (only when its config is not your own). The games tab is grouped by what each switch touches
- Reply from a notification for messengers that offer it (`inline-reply`)
- Session restore (off by default): the programs that were open come back on their workspaces at the next login
- `yami report` packs versions, doctor, this boot's journal and configs for an issue, with passwords, tokens, the home path, user and host scrubbed; `yami doctor` mentions btrfs/zfs snapshots (snapper, timeshift) before big updates
- Installer: two new components with flags, `tools` (night light, text and QR from screenshots: wlsunset, tesseract, zbar; `--no-tools`) and `games` (GameMode, MangoHud; on when Steam is installed, `--games` / `--no-games`). An update keeps the choice and turns a newly added component on at its default; since updates run without sudo, the packages a new version wants are listed in settings → updates with an install button, in `yami doctor` and in `yami update need [--run]`
- Settings: tabs sorted by meaning (screens next to windows), desk mode and title bars moved to windows, the cursor to look, the terminal font and fastfetch to fonts; the bar tab has two pages, modules and look; the night light says what it needs and hides the hours when it follows the sun; a long value no longer runs over its label
- Fix: Ctrl+A, Ctrl+C, Ctrl+V and Ctrl+U in shell fields and Ctrl+P in the clipboard work in any keyboard layout
- Fix: Screenshots on a fractional scale (1.25, 1.5) save exactly the selected area, marks and mosaic included, and the colour picker takes the pixel under the cursor
- Fix: Turning on «palette from the wallpaper» applies it at once instead of after the next wallpaper change
- Fix: A dock at the top that never hides no longer leaves a double gap under the bar, and it moves when its side changes
- Fix: Bar scripts stop together with everything they started and do not outlive a crashed shell; a long-running script is no longer cut off after 1 MB of output
- Fix: Restarting the shell (`yami restart`, every update) no longer leaves another copy of the music, sound and visualizer helpers (playerctl, pactl, parec) running or finished-but-unreaped each time
- Fix: yami builds on musl (Alpine): the glibc-only memory calls are made only with glibc
- Fix: The night light starts with a temperature of 6500 K and with equal on and off hours
- Fix: GameMode registration keeps the order of focus changes
- Fix: `yami msg` reports a command the shell does not have (`yami msg cal` used to answer «ok» and do nothing)
- Fix: `yami import` refuses an unknown flag instead of importing for real (`--dryrun`), writes the settings in the shell's own form (numbers no longer turn into 0.7799999713897705) and keeps its backups in the order they were taken
- Fix: `yami doctor` shows full program names (`xdg-desktop-portal`, `power-profiles-daemon`)
- Fix: niri reloads its config only when yami's part of it really changed
- Fix: An update writes the list of missing packages again; `install.sh --uninstall` also removes the light cursor and the MangoHud config it wrote
- Fix: Firefox menus got their shadow back (the CSS colour was written in the wrong order and was invisible)
- Security: `yami import` writes only the files `yami export` makes (a crafted file could drop anything into `~/.config`, autostart included); reassigned keys and the file manager's terminal come in only with `--yes`, and imported settings pass the same checks as the settings file
- Security: Key names, font names, app-ids and bar script names from the settings file are checked before they reach niri, kitty, GTK or Qt configs (a font name with a line break could add a command to kitty.conf through `yami theme import`)
- Security: `yami report` and screen text recognition use a private 0700 folder instead of a predictable one in /tmp, and the report hides coordinates, servers and key commands
- Updates: A failed build stops the update before anything else changes (the old version, its configs and the install record stay, so the update is offered again) and keeps the previous rollback point
- Updates: A new niri config that does not validate is replaced by the previous one, so the next login still has yami's keys
- Updates: The backup made before `yami import` holds every file the import may replace, is never overwritten by a second import in the same second, keeps the last five and goes back with `yami import FILE --yes`
- Updates: Session restore runs once per login even if the shell restarts
- Updates: An update of an install that has no record of its components (a home-only install of an older version) no longer starts a 5–25 minute yami-niri build unless yami-niri is already installed
- Updates: `yami clean`, `fork/yami-niri.sh clean` and the installer remove a `YAMI_NIRI_SRC` folder only when it holds nothing but the yami-niri trees; `install.sh --uninstall` also removes the build cache (gigabytes) unless it holds your patch work
- CI: every distro family the installer supports (Arch, Fedora, Debian, Ubuntu, openSUSE, Void, Alpine) checks the package names, builds the workspace and runs the installer's dry run; `install.sh --print-packages` prints the lists
- Code: app.rs, the file manager's fm.rs, draw.rs and app.rs, settings.rs, snap.rs and update.rs split into smaller modules; the updates tab shows release notes as plain text

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
