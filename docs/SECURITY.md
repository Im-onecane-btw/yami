# Security notes

What yami trusts, what it exposes, and what was checked (audit of 2026-11-03).

## Surfaces

| Surface | Who can reach it | Protection |
|---|---|---|
| `$XDG_RUNTIME_DIR/yami-shell.sock` (IPC) | processes of the same user | socket mode 0600, peer UID checked (`SO_PEERCRED`), one line ≤ 4 KB read with a 2 s timeout; the commands only open panels, lock, take screenshots or quit: none runs a program |
| polkit agent (`/org/yami/PolkitAgent`, system bus) | polkitd | calls from anyone but the owner of `org.freedesktop.PolicyKit1` are refused, so no other process can raise a password window; the password goes only to `polkit-agent-helper-1` and is wiped from memory after use |
| lock screen | the seat | `ext-session-lock`: if the shell dies, niri keeps the screen locked. Password checked by PAM (`pam_unix` only, own service file in `~/.local/state/yami/pam`), copies wiped after the check |
| privacy curtain (Mod+Shift+H) | xdg-desktop-portal screencasts (OBS PipeWire, Discord, browsers) | on yami-niri ≥ `.4` a layer only screencasts draw (`show-only-in`) covers the screen with a frozen frame, blurred (or in 24 px pixels, or the wallpaper); shell panels opened meanwhile get a new curtain above them. niri's Alt+Tab switcher draws above every layer and is not covered; wlr-screencopy capture (grim, OBS wlrobs) is not covered. On other niri the windows are blocked out instead |
| portal backends (FileChooser, ScreenCast, FileManager1) | xdg-desktop-portal, same-user apps on the session bus | sandboxed apps only reach them through the portal frontend; the screen picker is the shell's own modal |
| notifications, tray | same-user apps | rendered as text and decoded images (pure Rust decoders); nothing is executed except an action the user clicks |
| opening files | the user's clicks | `Exec=` lines are split into argv without a shell, file names are never interpolated into a command line |
| custom keybindings | the user (settings → keys) | run with `sh -c` by niri, like any niri `spawn-sh` |

## Installer

- Runs as the user; `sudo`/`doas` only for packages, services, the SDDM theme, the NVIDIA profile and,
  when the user has no realtime limit, `setcap cap_sys_nice=ep` on the yami-niri binary (the
  compositor drops every capability right after taking its scheduling class; any write to the file
  clears the capability).
- Downloads: the repository itself (git over HTTPS), rustup when the distro's Rust is too old
  (`https` only, TLS 1.2+), Monocraft at a pinned version checked against its SHA-256. niri and
  smithay are fetched at pinned commits, the patches come from this repository.
- `bash <(curl …)` runs code from the network: read `install.sh` first if you do not trust the
  source; `--dry-run` shows the plan.

## Known trade-offs

- The SDDM theme's `theme.conf.user` and `background.png` belong to the installing user so the
  palette and wallpaper can follow the shell; on a machine shared with untrusted users, make them
  root-owned (`tools/sddm-install.sh` then needs re-running after palette changes).
- The lock screen's PAM service file lives in the user's state directory: a process running as
  the user could weaken it, but such a process already controls the session.
- Clipboard history (cliphist) skips entries password managers mark as secret
  (`x-kde-passwordManagerHint`, wl-clipboard's `CLIPBOARD_STATE=sensitive`); anything else copied
  is stored until deleted (`Mod+V`, `Del`).
