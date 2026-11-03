# yami-niri

Our niri: upstream [niri](https://github.com/YaLTeR/niri) at a pinned release plus a short
patch series, built on the machine it runs on. The installer builds and enables it by default;
the packaged niri stays installed as the fallback.

| What | Where |
|---|---|
| `yami-niri.sh build \| on \| off \| status \| clean` | fetch, patch, build, enable from the next login |
| `session.sh` | the session's `ExecStart` (systemd drop-in): starts the fork, falls back to the packaged niri if it rejects the config or dies within 15 s |
| `patches/niri/*.patch` | on top of `NIRI_REV` (v26.04) |
| `patches/smithay/*.patch` | on top of `SMITHAY_REV` (the rev niri v26.04 pins) |

Installed layout: `~/.local/lib/yami-niri/<rev>/niri`, `current` → the live build, `session`,
`yami-niri.sh`; `~/.local/bin/niri` → the fork (so `niri msg` matches the compositor);
`~/.config/systemd/user/niri.service.d/yami-niri.conf`. Sources and the build cache:
`~/.cache/yami/niri-src` (`yami-niri.sh clean` drops them).

## Patches

1. **Build profile** `yami`: fat LTO, one codegen unit, no debuginfo, `-C target-cpu=native`
   (and mold when present). Overflow checks stay on, as upstream.
2. **Tearing for games** (smithay + niri): window rule `allow-tearing true`. While such a window
   is fullscreen on top of its output and the view is still, a frame that changes nothing but the
   primary plane's buffer is flipped with `DRM_MODE_PAGE_FLIP_ASYNC`: shown at once instead of at
   the next vblank, less input lag. The desktop never tears; a refused async flip falls back to a
   normal one. The shell writes the rule (settings → games) only when it runs on yami-niri.
3. **Realtime main thread**: `SCHED_RR` 1 with `SCHED_RESET_ON_FORK` when the user may
   (`RLIMIT_RTPRIO`, e.g. the `realtime`/`audio` group, or `setcap cap_sys_nice=ep` on the binary,
   which the installer does when needed). A game loading every core no longer makes the cursor and
   windows stutter. Capabilities are dropped right after; threads and spawned programs start with
   the normal policy. `YAMI_NIRI_RT=0` turns it off.

4. **Layer rule `show-only-in`**: the opposite of `block-out-from`. A layer surface with
   `show-only-in "screencast"` (or `"screen-capture"`) is drawn only to those targets, never on
   the monitor. The shell's privacy curtain (Mod+Shift+H) uses it: stream viewers see a cover,
   you see the desktop. The look is the shell's; the compositor only decides where it shows.
5. **Layer rule `hide-from`, covers over window casts**: like `block-out-from`, but the surface
   is not drawn to those targets at all instead of as a black box over its whole rectangle (shadow
   margins included). The shell hides its notifications and the curtain's «hidden» badge from
   recordings with it. A window screencast draws `show-only-in "screencast"` layers over the
   window too, shifted to where the window is on the monitor, so the curtain also covers a single
   shared window.

`niri --version` says `26.04-yami.N (<rev>)`, where N is the series level (the number of patches).
The shell detects the fork by `yami` and writes a fork-only key only when the running build's
level has it (`show-only-in` from 4, `hide-from` from 5): right after a build the old binary runs until the next login.

## Rules

- Keep the series small and upstream-shaped: every patch is one feature with a reason, and must
  apply to the pinned revs. No effects or eye candy in the compositor; the shell draws the look.
- The config must stay valid for the packaged niri too, except for what the shell writes when it
  sees the fork (`allow-tearing`, `show-only-in`, `hide-from`); `session.sh` strips or downgrades
  them before falling back.
- A patch that adds config the shell writes bumps the level in `VERSION`.
- Updating: bump `NIRI_REV`/`SMITHAY_REV` in `yami-niri.sh`, rebase the series in
  `~/.cache/yami/niri-src`, re-export with
  `git format-patch -o fork/patches/niri <rev>..yami` (and the same for smithay), build, run
  `cargo test` in the niri tree, then test a login (Ctrl+Alt+F3 → `yami-niri.sh off` if it breaks).
- Not taken from angel-niri (the previous base): desk throw, glass cracks, glitch shaders, pixel
  overview, clipboard keep, privacy look, stacking with server-side title bars. Minimizing and the
  «desk» mode are done by the shell on any niri.
