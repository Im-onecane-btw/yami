# Security notes

English · [Русский](SECURITY.ru.md)

What yami trusts, what it exposes, and what was checked (audits of 2026-11-03 and 2026-11-04).

## Surfaces

| Surface | Who can reach it | Protection |
|---|---|---|
| `$XDG_RUNTIME_DIR/yami-shell.sock` (IPC) | processes of the same user | socket mode 0600, peer UID checked (`SO_PEERCRED`), one line ≤ 4 KB read with a 2 s timeout; the commands only open panels, lock, take screenshots or quit: none runs a program |
| polkit agent (`/org/yami/PolkitAgent`, system bus) | polkitd | calls from anyone but the owner of `org.freedesktop.PolicyKit1` are refused, so no other process can raise a password window; the password goes only to `polkit-agent-helper-1` and is wiped from memory after use |
| lock screen | the seat | `ext-session-lock`: if the shell dies, niri keeps the screen locked. Password checked by PAM (`pam_unix` only, own service file in `~/.local/state/yami/pam`, so faillock never locks the account over typos; a user not in `/etc/passwd` — LDAP, SSSD, systemd-homed — goes through the system's `system-local-login` or `login` service instead), copies wiped after the check; a monitor plugged in while locked gets its own lock surface |
| privacy curtain (Mod+Shift+H) | xdg-desktop-portal screencasts (OBS PipeWire, Discord, browsers) | on yami-niri ≥ `.4` a layer only screencasts draw (`show-only-in`) covers the screen with a frozen frame, blurred (or in 24 px pixels, or the wallpaper); shell panels opened meanwhile get a new curtain above them; from `.5` a single shared window is covered too (its part of the curtain). niri's Alt+Tab switcher draws above every layer and is not covered; wlr-screencopy capture (grim, OBS wlrobs) is not covered. On other niri the windows are blocked out instead |
| portal backends (FileChooser, ScreenCast, FileManager1) | xdg-desktop-portal, same-user apps on the session bus | sandboxed apps only reach them through the portal frontend; the screen picker is the shell's own modal |
| notifications, tray | same-user apps | rendered as text and decoded images (pure Rust decoders); raw pictures are size-checked before allocation (≤ 1024², no integer overflow), files named by an app (`image-path`, cover `file://`) are read only if regular and ≤ 16 MB, decoders run with a 64 MB cap; at most 64 tray items; nothing is executed except an action the user clicks; each service can be turned off (settings → services) |
| album covers | the player (MPRIS `artUrl`) | `curl -q` (no user curlrc), http(s) only, https after redirects, ≤ 3 redirects, ≤ 16 MB, 8 s; `mediaArtNet` off keeps the shell offline |
| lyrics | the player (MPRIS `xesam:asText`, `xesam:url`), lrclib.net | the player's text and a `.lrc`/`.txt` next to a local track are read as text (regular files, ≤ 256 KB); lrclib.net gets artist, title and length over `curl -q`, https only, 8 s, ≤ 2 MB, parsed as JSON and shown as text; asked only while the music popup shows lyrics; `lyricsNet` off keeps it offline |
| file manager | removable media | a `.trashinfo` on a removable disk cannot point outside that disk (`..` is refused); archives go through `bsdtar`, which refuses absolute paths and `..`; FileManager1 opens at most 16 folders per call |
| image viewer | files you open | animations are capped at 600 frames and 1 GB of decoded frames |
| wallpaper picker | pictures in the wallpaper folder | only regular, not hidden png/jpg/webp files, at most 500 per folder; decoded in a background thread of the shell with the same limits as yami-files (nothing over 40 Mpx, a 256 MB decode cap), thumbnails cached in `~/.cache/yami/thumbs`; only the rows on screen stay in memory and everything is freed when it closes |
| opening files | the user's clicks | `Exec=` lines are split into argv without a shell, file names are never interpolated into a command line |
| custom keybindings | the user (settings → keys) | run with `sh -c` by niri, like any niri `spawn-sh` |
| bar scripts (`~/.config/yami/modules`) | the user's own executables | run only while their module is on the bar, each in its own process group led by a small `sh`: the whole group (the script and what it started) gets SIGTERM when the module leaves the bar or the shell dies (`PR_SET_PDEATHSIG` on the leader); output read line by line (a line over 4 KB is skipped); a module's `click` runs with `sh -c` like a custom key |
| launcher `>` and `/` | what the user types | `>` runs the typed command in a terminal (like typing it there); `/` only walks the home folder (no hidden folders, no symlinked folders, ≤ 8 levels, ≤ 60 000 entries) and opens the choice with `xdg-open` |
| inline reply | the notifying app | the text goes back only as `NotificationReplied` to that notification; the field is wiped after sending, nothing is stored or copied |
| `yami theme import` | a file or an https link from anyone | a link is fetched with `curl -q`, https only (redirects too), ≤ 3 redirects, ≤ 64 MB, 30 s, into a fresh 0700 folder, and must be a theme (`kind: theme`), never a full file; only look keys (palette, effects, fonts, bar, widgets, dock look, gaps, cursor) and the wallpaper picture (saved as a plain file under `~/.local/share/yami/wallpapers`, name stripped of any path); never keybindings, commands, scripts or the update source. Before any import everything it may replace is copied to `~/.local/state/yami/before-import-*.json` (the same format, the last five kept; `yami import FILE --yes` puts it back) |
| `yami import` | your own backup | as above plus every other setting; custom keybindings, `niri/user.kdl` and bar scripts are listed and written only with `--yes`; paths must be plain and relative inside `~/.config` (`..` and absolute paths refused); `updateRepo` and `updateVerify` are never imported |
| `yami report` | you, for an issue | nothing is sent; lines with password, token, secret or key names are blanked, the home path, user and host name replaced; the archive is meant to be read before it is attached |
| shell-owned processes | the user's settings | wlsunset (night light) is started with fixed arguments; it and the music, sound and visualizer helpers (playerctl, pactl, parec) carry `PR_SET_PDEATHSIG`, so none outlives the shell or a restart |

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

## Updates

- The updater works in its own clone (`~/.local/share/yami/src`), runs git without prompts
  (`GIT_TERMINAL_PROMPT=0`, hooks off) and with a stall timeout; tags are fetched without
  `--force`, so a tag rewritten on the server does not replace one already seen.
- Signatures: the trusted keys come from `keys/allowed_signers` of the version that is already
  installed (trust on first install, like the install itself). Once that file holds keys, a
  release whose tag is unsigned or signed by another key is refused (`updateVerify` `auto`), so
  someone who gains push access to the repository still cannot ship an update. A signature only
  counts with a matching principal (`gpg.minTrustLevel=fully`, and «No principal matched» is
  refused even where an old git would exit 0). Releases are cut
  with `tools/release.sh`, which refuses to make an unsigned tag once keys exist.
- Installing: `install.sh --update` from the new tree as the user, no sudo; the previous binaries
  are kept for `yami update rollback`. A game in focus postpones the shell restart.
- Private or public repository: the same HTTPS URL. While it is private, git uses the user's
  credential helper (`gh auth login`); publishing the repository needs no change on clients.

## Known trade-offs

- The SDDM theme's `theme.conf.user` and `background.png` belong to the installing user so the
  palette and wallpaper can follow the shell; on a machine shared with untrusted users, make them
  root-owned (`tools/sddm-install.sh` then needs re-running after palette changes).
- The lock screen's PAM service file lives in the user's state directory: a process running as
  the user could weaken it, but such a process already controls the session.
- Clipboard history (cliphist) skips entries password managers mark as secret
  (`x-kde-passwordManagerHint`, wl-clipboard's `CLIPBOARD_STATE=sensitive`); anything else copied
  is stored until deleted (`Mod+V`, `Del`).

## Install count

yami has no telemetry: the shell never reports anything, and updates are a `git fetch` of the
public repository. The «installs» badge in the README counts one thing: the installer downloads a
tiny file, `yami-installs.txt`, from the latest GitHub release **once per machine** and throws it
away. GitHub counts downloads of release files; `.github/workflows/stats.yml` adds those counts up
daily and writes the badge to the `stats` branch.

- The request carries nothing about you or the machine: no id, no version, no settings. GitHub
  sees what it sees on any download (your IP address), as with the `git clone` the installer
  already does.
- Once means once: a marker `~/.local/state/yami/counted` keeps later updates silent. Machines
  installed before the counter are counted once, on their next update.
- Opt out: `install.sh --no-count`, or `YAMI_NO_COUNT=1` in the environment (also skipped in CI
  and `--dry-run`). Creating the marker file by hand does the same.
- It counts installs, not people and not active machines: a reinstall after deleting
  `~/.local/state/yami` counts again, an uninstall does not subtract.
