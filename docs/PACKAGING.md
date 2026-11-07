# Packaging

English · [Русский](PACKAGING.ru.md)

Notes for Nix users and distribution packagers: the flake, where yami looks for system files, and how to hand upgrades to the package manager.

## Nix

The flake has the package (the shell, yami-files, yami-view, yami-play and the login screen yami-greet), a NixOS module and a Home Manager module. The NixOS module is the system half of `install.sh`: niri, the services the shell talks to, the portals with yami's file dialog and screen-share picker, fonts, the commands the shell runs and the login screen. The Home Manager module is the home half: niri's config, the shell's settings, kitty, fontconfig and the default apps.

```nix
{
  inputs.yami.url = "github:Trendorin/yami";
  inputs.yami.inputs.nixpkgs.follows = "nixpkgs";

  # NixOS
  imports = [ yami.nixosModules.default ];
  programs.yami.enable = true;
  programs.yami.loginScreen.user = "alice"; # whose shell gives the login screen its look

  # Home Manager
  imports = [ yami.homeManagerModules.default ];
  programs.yami.enable = true;
  programs.yami.settings = { palette = "sakura"; lang = "en"; };
}
```

NixOS options:

| Option | Default | What |
|---|---|---|
| `programs.yami.enable` | `false` | niri, NetworkManager, BlueZ, udisks, gvfs, polkit, power profiles, the keyring, the portals, fonts and the shell's commands |
| `programs.yami.loginScreen.enable` | `true` | greetd starts yami-greet inside niri; off keeps another display manager |
| `programs.yami.loginScreen.user` | `null` | the user whose shell writes the login screen's look (palette, wallpaper, language, screens, keyboard layouts) |
| `programs.yami.apps` | `true` | kitty, fastfetch, pavucontrol, btop, video thumbnails, bsdtar |
| `programs.yami.tools` | `true` | night light, text and QR codes from screenshots |
| `programs.yami.appTheme` | `true` | qt6ct, adw-gtk3 and Papirus for Qt and GTK apps in the palette |

Home Manager options: `settings` (keys of `yami-shell.json`, merged on every activation: these win, the rest set in the settings panel stays), `niriConfig` (yami's `config.kdl`, copied while `~/.config/niri/.yami` marks it as yami's; your own changes go into `user.kdl`), `kitty`, `defaultApplications`, `extraPackages`.

The shell writes its own files at run time (settings, keys, screens, portals), so the modules seed them once and leave them writable. yami-niri, the patched niri of `fork/`, is not packaged: the packaged niri is the same desktop without tearing in games, the realtime main thread and the stream curtain's layer rules. `nix flake check` builds the package and evaluates the NixOS module; CI runs it on every push. The modules started from the work of TheCodedKid in #1.

## System files

The shell finds the polkit helper and the gvfs MTP backend without fixed FHS paths. On NixOS the polkit helper is the setuid wrapper `/run/wrappers/bin/polkit-agent-helper-1`, which comes first: a copy of the helper from `PATH` has no setuid bit and cannot check a password. gvfs is found through `XDG_DATA_DIRS`. The old paths under `/usr` stay as fallbacks.

The login screen: greetd runs `libexec/yami/greet-session` as its greeter user (the installer puts it in `/usr/local/lib/yami`). It finds `bin/yami-greet` from its own folder (`YAMI_GREET` names another), niri in `PATH`, the sessions in the `wayland-sessions` folders of `XDG_DATA_DIRS`, and yami-greet finds its fonts in `share/yami-greet/fonts` next to its binary (`YAMI_GREET_FONTS`). It needs three folders: `/var/lib/yami-greet/look` owned by the user whose shell publishes the look, `/var/lib/yami-greet/state` (0700) and `/var/lib/yami-greet/log` owned by the greeter user.

## Updates by the package manager

Build with the Cargo feature `package-managed` (the flake does):

```sh
cargo build --release --locked --workspace --features yami-shell/package-managed --manifest-path shell/Cargo.toml
```

Such a build never checks for updates on a schedule, the updates tab shows a note instead of its buttons, and `yami update` (with any subcommand) prints «Updates are managed by your package manager.» and exits with 1. Update, release switch and rollback refuse before they touch the source clone or the installed binaries. Upgrade and roll back with the package manager. A build without the feature keeps the updater.

Do not run `install.sh` over an installation that the package manager owns.
