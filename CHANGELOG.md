# Changelog

What changed in each release, newest first. `yami update` shows the section of the release it
offers; `tools/release.sh X.Y.Z` turns «Unreleased» into the release (or writes it from the commit
subjects when that section is empty).

## Unreleased

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
