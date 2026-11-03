# Changelog

What changed in each release, newest first. `yami update` shows the section of the release it
offers; `tools/release.sh X.Y.Z` turns «Unreleased» into the release (or writes it from the commit
subjects when that section is empty).

## Unreleased

- `yami`: one command for the whole desktop (open, palette, wallpaper, restart, status, doctor, update)
- updates from settings or `yami update`: releases or every commit, signed tags, rollback
- settings → services: the notification server, tray, polkit window, portals and covers can be turned off
- notification and tray pictures are size-checked; covers and icon paths are read with a cap
- the album-cover cache no longer grows with every track

## 2.0.0

- the yami desktop: shell, yami-files, yami-view, yami-niri, SDDM theme, multi-distro installer
