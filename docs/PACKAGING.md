# Packaging

English · [Русский](PACKAGING.ru.md)

Notes for distribution and Nix packagers: where yami looks for system files, and how to hand upgrades to the package manager.


## System files

The shell finds the polkit helper and the gvfs MTP backend without fixed FHS paths. On NixOS the polkit helper is the setuid wrapper `/run/wrappers/bin/polkit-agent-helper-1`, which comes first: a copy of the helper from `PATH` has no setuid bit and cannot check a password. gvfs is found through `XDG_DATA_DIRS`. The old paths under `/usr` stay as fallbacks.

The login screen: greetd runs `libexec/yami/greet-session` as its greeter user (the installer puts it in `/usr/local/lib/yami`). It finds `bin/yami-greet` from its own folder (`YAMI_GREET` names another), niri in `PATH`, the sessions in the `wayland-sessions` folders of `XDG_DATA_DIRS`, and yami-greet finds its fonts in `share/yami-greet/fonts` next to its binary (`YAMI_GREET_FONTS`). It needs three folders: `/var/lib/yami-greet/look` owned by the user whose shell publishes the look, `/var/lib/yami-greet/state` (0700) and `/var/lib/yami-greet/log` owned by the greeter user.

## Updates by the package manager

Build with the Cargo feature `package-managed`:

```sh
cargo build --release --locked --workspace --features yami-shell/package-managed --manifest-path shell/Cargo.toml
```

Such a build never checks for updates on a schedule, the updates tab shows a note instead of its buttons, and `yami update` (with any subcommand) prints «Updates are managed by your package manager.» and exits with 1. Update, release switch and rollback refuse before they touch the source clone or the installed binaries. Upgrade and roll back with the package manager. A build without the feature keeps the updater.

Do not run `install.sh` over an installation that the package manager owns.
