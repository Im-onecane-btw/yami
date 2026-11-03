# Changelog

What changed in each release, newest first. `yami update` shows the section of the release it
offers; `tools/release.sh X.Y.Z` turns «Unreleased» into the release (or writes it from the commit
subjects when that section is empty).

## Unreleased

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
