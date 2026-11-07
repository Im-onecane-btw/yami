#!/usr/bin/env bash
# YAMI installer: the yami desktop on niri (the shell, files, viewer, player, login screen, themes
# and fonts) on any mainstream Linux distro. One command, from anywhere, as yourself (not root):
#
#   bash <(curl -fsSL https://raw.githubusercontent.com/Trendorin/yami/main/install.sh)
#
# It asks the language, shows what it will install and goes on Enter («c» changes the choice). Every
# default is picked for this machine: nothing below is needed for a normal install.
#
#   ./install.sh --yes            no questions, the defaults
#   ./install.sh --dry-run        show the plan, change nothing
#   ./install.sh --uninstall      remove the shell and its files, restore what it replaced
#   ./install.sh --update         rebuild from this tree, keep every setting, no sudo (what
#                                 `yami update` runs after checking the release)
#
# Choices (the interactive installer asks them; settings change them later):
#   --lang en|ru|pt-BR     --palette blood|ash|sakura|amber|venom|ice|dusk     --theme dark|light
#   --look matte|glass|light|solid     --font sans|pixel     --fetch mini|full
#
# Components (on by default when they fit this machine):
#   --no-fork        packaged niri instead of yami-niri (built from source, 5–25 min; off by
#                    default with less than 6 GB of memory or 4 threads, --fork turns it on)
#   --no-login       leave the login manager alone (--login: ours even if another one is set)
#   --no-games       no GameMode and MangoHud (--games: even without Steam)
#   --no-tools       no night light, screen text and QR (wlsunset, tesseract, zbar)
#   --no-sudo        the home folder only: no packages, no system files
#
# Rarely needed: --no-monocraft (skip the 31 MB terminal font), --no-count (do not count this
# install, docs/SECURITY.md), --keep-build (keep the ~4 GB yami-niri sources), --no-anim,
# --print-packages (the package lists for this distro, CI checks them).
#
# Families: Arch, Fedora (incl. atomic), Debian/Ubuntu, openSUSE, Void, Alpine, Gentoo. NixOS: the
# flake (flake.nix, docs/PACKAGING.md); this script installs only the home folder there. Anything
# replaced is kept next to itself as *.bak-DATE. Log: ~/.cache/yami/install.log. The steps live in
# installer/*.sh; this file is the order they run in.
set -uo pipefail
# the whole script is one block and the modules are read whole when sourced: bash parses all of it
# before running, so an update of these files mid-run (git pull, an editor) cannot shift lines
{

# ── bootstrap: started outside the repo (curl | bash) — fetch it and run from there ──
YAMI_REPO="${YAMI_REPO:-https://github.com/Trendorin/yami.git}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd)"
if [[ ! -f $ROOT/shell/Cargo.toml || ! -f $ROOT/installer/ui.sh ]]; then
  SRC_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/yami/src"
  command -v git >/dev/null || { echo "yami: git is needed to fetch the installer (install git first)" >&2; exit 1; }
  if [[ -d $SRC_DIR/.git ]]; then
    # a clone of an older address follows the current one
    [[ "$(git -C "$SRC_DIR" remote get-url origin 2>/dev/null)" == "$YAMI_REPO" ]] || git -C "$SRC_DIR" remote set-url origin "$YAMI_REPO"
    git -C "$SRC_DIR" pull --ff-only --quiet || echo "yami: could not update $SRC_DIR, using it as is" >&2
  else
    mkdir -p "${SRC_DIR%/*}"
    # history without file contents: small like --depth 1, but updates can compare versions
    git clone --filter=blob:none "$YAMI_REPO" "$SRC_DIR" || { echo "yami: git clone $YAMI_REPO failed" >&2; exit 1; }
  fi
  # curl … | bash: give the interactive installer the terminal back
  if [[ ! -t 0 ]] && { : </dev/tty; } 2>/dev/null; then exec bash "$SRC_DIR/install.sh" "$@" </dev/tty; fi
  exec bash "$SRC_DIR/install.sh" "$@"
fi

# ── where things go ─────────────────────────────────────────────────────────
CFG="${XDG_CONFIG_HOME:-$HOME/.config}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/yami"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}/yami"
BIN="$HOME/.local/bin"
STAMP="$(date +%Y%m%d-%H%M%S)"
# settings of this shell; an install under the old name (DARK-4RMY) is read until it is moved
SET_JSON="$CFG/yami-shell.json"
[[ ! -f $SET_JSON && -f $CFG/dark4rmy-shell.json ]] && SET_JSON="$CFG/dark4rmy-shell.json"
LOG="$CACHE/install.log"
MISSING_F="$CACHE/missing-packages"
RUST_MIN="1.89"
NIRI_TAG="v26.04"
# ext-background-effect (blur only under the shell panels, not their shadow margins) and the
# config syntax need niri 26.04 or newer
NIRI_MIN="26.04"
# pinned and checked: a changed upload is refused (kitty then uses Terminess from the repo)
MONOCRAFT_URL="https://github.com/IdreesInc/Monocraft/releases/download/v4.2.1/Monocraft-nerd-fonts-patched.ttc"
MONOCRAFT_SHA256="67f88ff9e7c6560f6cf60fb062fd353a72f62dc2654462950c65b63ed53d9754"
[[ -d $HOME/.cargo/bin ]] && export PATH="$HOME/.cargo/bin:$PATH"

# ── flags ───────────────────────────────────────────────────────────────────
YES=0 SUDO=1 FORK_FLAG="" LOGIN_FLAG="" GAMES_FLAG="" TOOLS_FLAG=1 PALETTE="" LOOK_OPT="" THEME_OPT="" LANG_OPT="" FONT_OPT="" FETCH_OPT="" ANIM=1 DRY=0 UNINSTALL=0 UPDATE=0 COUNT=1 KEEP_BUILD=0 MONO="" PRINT_PKGS=0
while (($#)); do
  case "$1" in
    -y | --yes) YES=1 ;;
    --no-sudo) SUDO=0 ;;
    --fork) FORK_FLAG=1 ;;
    --no-fork) FORK_FLAG=0 ;;
    --games) GAMES_FLAG=1 ;;
    --no-games) GAMES_FLAG=0 ;;
    --tools) TOOLS_FLAG=1 ;;
    --no-tools) TOOLS_FLAG=0 ;;
    --login) LOGIN_FLAG=1 ;;
    --no-login) LOGIN_FLAG=0 ;;
    --font) FONT_OPT="${2:-}"; shift ;;
    --fetch) FETCH_OPT="${2:-}"; shift ;;
    --palette) PALETTE="${2:-}"; shift ;;
    --look) LOOK_OPT="${2:-}"; shift ;;
    --theme) THEME_OPT="${2:-}"; shift ;;
    --lang) LANG_OPT="${2:-}"; shift ;;
    --no-anim) ANIM=0 ;;
    --dry-run) DRY=1 ;;
    --print-packages) PRINT_PKGS=1 ;;
    --no-count) COUNT=0 ;;
    --keep-build) KEEP_BUILD=1 ;;
    --no-monocraft) MONO=0 ;;
    --monocraft) MONO=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --update) UPDATE=1 YES=1 ANIM=0 SUDO=0 ;;
    -h | --help) sed -n '2,/^set -uo/p' "${BASH_SOURCE[0]:-$0}" | sed -e '$d' -e 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done
[[ -t 0 && -t 1 ]] || { YES=1; ANIM=0; }
# git never asks for a login on the terminal: a moved or private repo fails instead of hanging
export GIT_TERMINAL_PROMPT=0

# ── the modules: each only defines (installer/*.sh) ─────────────────────────
for m in ui detect root packages components menu build desktop niri login nvidia migrate uninstall; do
  # shellcheck source=/dev/null
  . "$ROOT/installer/$m.sh" || { echo "yami: $ROOT/installer/$m.sh is missing" >&2; exit 1; }
done
unset m

detect_distro
if ((PRINT_PKGS)); then
  packages_for_family
  print_packages
  exit 0
fi
# `sudo ./install.sh` would put the desktop into root's home: the installer asks for the password
# itself, only where it is needed (containers and CI run as root on purpose: YAMI_AS_ROOT=1)
if ((EUID == 0)) && [[ -z ${YAMI_AS_ROOT:-} && -z ${CI:-} ]]; then
  echo "yami: run the installer as yourself, not as root: it asks for the password when it needs it" >&2
  echo "      ./install.sh   (a root shell on purpose, e.g. a container: YAMI_AS_ROOT=1)" >&2
  exit 1
fi
mkdir -p "$CACHE"
: >"$LOG"
: >"$MISSING_F"

# ── language: the flag, then what the shell is set to (update), then the system ──
L=""
if [[ -n $LANG_OPT ]]; then
  L="$(lang_code "$LANG_OPT")"
  [[ -n $L ]] || { echo "--lang: en, ru or pt-BR" >&2; exit 2; }
elif ((UPDATE)) && [[ -f $SET_JSON ]]; then
  L="$(lang_code "$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("lang",""))' "$SET_JSON" 2>/dev/null)")"
fi
if [[ -z $L ]]; then
  L="$(lang_code "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}")"
  L="${L:-en}"
  LANG_ASK=1
else
  LANG_ASK=0
fi
# ${#s} must count characters, not bytes
case "$(locale charmap 2>/dev/null)" in UTF-8) ;; *) export LC_ALL=C.UTF-8 ;; esac

detect_machine
detect_login
packages_for_family
choices_init
paint
ui_traps
components_init
components_limit

# ── go ──────────────────────────────────────────────────────────────────────
((UNINSTALL)) && uninstall

if ((UPDATE)); then
  # rebuild from this tree with the choices of the first install; no packages, no sudo
  say "$(t "обновление из" "update from" "atualizar de") $ROOT"
  add "$(t "шелл: сборка из исходников" "shell: build from source" "shell: compilar do código-fonte")" do_build_shell
  add "$(t "файлы: yami-files и диалог выбора файлов" "files: yami-files and the file dialog" "arquivos: yami-files e diálogo de arquivos")" do_files
  add "$(t "шрифты" "fonts" "fontes")" do_fonts
  add "$(t "niri: конфиг" "niri: config" "niri: configuração")" do_niri_config
  add "$(t "палитра: всё, что из неё выводится" "palette: everything derived from it" "paleta: tudo deriva dela")" do_refresh
  add "$(t "следы прошлых версий" "leftovers of earlier versions" "restos de versões anteriores")" do_legacy
  add "$(t "запись об установке" "install record" "registro de instalação")" do_stamp
  # an update has no sudo: what the chosen components still miss goes to packages-needed, and the
  # shell offers one command for it (settings → updates, yami doctor, yami update need)
  add "$(t "каких пакетов ещё не хватает" "packages still missing" "pacotes ainda ausentes")" do_need_pkgs
  add "$(t "счётчик установок (один раз, без данных)" "install counter (once, no data)" "contador de instalações (uma vez, sem dados)")" do_count
  ((RUNNING_NIRI)) && add "$(t "перезапуск шелла" "restart the shell" "reiniciar o shell")" do_restart_shell
  run_steps
  if on fork && ((!DRY && ${#FAILED[@]} == 0)) && fork_stale; then
    say "$(t "yami-niri: новая серия патчей — сборка (5–25 минут)" "yami-niri: a new patch series, building (5–25 minutes)" "yami-niri: nova série de patches, compilando (5–25 minutos)")"
    fork_build || FAILED+=("yami-niri")
  fi
  login_offer
  if ((${#FAILED[@]})); then
    warn "$(t "не вышло:" "failed:" "falhas:") ${FAILED[*]}"
    note "$(t "подробности:" "details:" "detalhes:") $LOG"
    exit 1
  fi
  say "$(t "обновлено" "updated" "atualizado")"
  exit 0
fi

if ((!YES)); then
  printf '\e[H\e[J'
  banner 1
  ask_all
fi
paint
[[ -t 1 ]] && printf '\e[H\e[J'
banner $((YES))

if ((SUDO && !DRY)) && [[ -n $SU ]]; then
  say "$(t "нужен пароль администратора — один раз на всю установку" "the administrator password, once for the whole install" "a senha de administrador, uma vez para toda a instalação")"
  if ! root_start; then
    if $SU true; then
      # the password is right but the root shell did not come up (an unusual sudo setup): every
      # root command asks on its own, the steps run in the foreground so the prompts are seen
      warn "$(t "общий root-сеанс не поднялся — пароль может спрашиваться не раз" "the shared root session did not start: the password may be asked more than once" "a sessão root compartilhada não iniciou: a senha pode ser solicitada mais de uma vez")"
    else
      warn "$(t "без root — ставлю только домашнюю папку" "no root: home directory only" "sem root: apenas a pasta pessoal")"
      SUDO=0
      for i in "${!C_ID[@]}"; do can "${C_ID[$i]}" || C_ON[i]=0; done
    fi
  fi
  printf '\n'
fi

# root steps first: the root shell closes before the build (run_steps), see installer/root.sh
if ((SUDO)) && { on core || on apps || on look; }; then
  add "$(t "пакеты: обновить списки" "packages: refresh index" "pacotes: atualizar índice")" pm_refresh
fi
on core && add "$(t "пакеты: niri" "packages: niri" "pacotes: niri")" pkgs "${P_niri[@]}"
on core && add "$(t "пакеты: сеанс и шелл" "packages: session and shell" "pacotes: sessão e shell")" pkgs "${P_base[@]}"
# a working toolchain (rustup included) is kept: only the libraries and pkg-config are added
P_build_need=()
for p in "${P_build[@]}"; do
  case "$p" in rust | rustc | cargo | virtual/rust) rust_ok && continue ;; esac
  P_build_need+=("$p")
done
((SUDO)) && add "$(t "пакеты: сборка шелла" "packages: shell build deps" "pacotes: dependências de compilação do shell")" pkgs "${P_build_need[@]}"
on apps && add "$(t "пакеты: программы" "packages: apps" "pacotes: aplicativos")" pkgs "${P_apps[@]}"
on look && add "$(t "пакеты: тема Qt и GTK" "packages: Qt and GTK theme" "pacotes: tema Qt e GTK")" pkgs "${P_look[@]}"
on tools && add "$(t "пакеты: ночной свет, текст и QR со скриншота" "packages: night light, text and QR from screenshots" "pacotes: luz noturna, texto e QR de capturas de tela")" pkgs "${P_tools[@]}"
on games && add "$(t "пакеты: GameMode, MangoHud" "packages: GameMode, MangoHud" "pacotes: GameMode e MangoHud")" pkgs "${P_games[@]}"
on login && add "$(t "пакеты: greetd" "packages: greetd" "pacotes: greetd")" pkgs "${P_login[@]}"
# niri's build libraries: for yami-niri, or for upstream niri when the packaged one is too old
if ((SUDO)) && { on fork || { on core && ! niri_ok && ((!DRY)); }; }; then
  add "$(t "пакеты: сборка niri" "packages: niri build deps" "pacotes: dependências de compilação do niri")" pkgs "${P_nirisrc[@]}"
fi
on core && add "$(t "niri $NIRI_MIN+ из исходников, если пакет старее" "niri $NIRI_MIN+ from source if the package is older" "niri $NIRI_MIN+ compilado se o pacote for mais antigo")" do_niri_source
on nvidia && add "$(t "NVIDIA: профиль против утечки VRAM" "NVIDIA: VRAM leak profile" "NVIDIA: perfil contra vazamento de VRAM")" do_nvidia_profile
on nvidia && add "$(t "NVIDIA: картинка с начала загрузки (пароль диска на любом выходе)" "NVIDIA: picture from the start of boot (disk password on any output)" "NVIDIA: imagem desde o início da inicialização (senha do disco em qualquer saída)")" do_nvidia_early
on nvidia && add "$(t "NVIDIA: кодек видеокарты в OBS на старом драйвере (GStreamer)" "NVIDIA: the GPU encoder in OBS on an old driver (GStreamer)" "NVIDIA: codificador da GPU no OBS com driver antigo (GStreamer)")" do_nvidia_obs
on core && ((SUDO)) && add "$(t "службы: bluetooth, профили питания, сеть" "services: bluetooth, power profiles, network" "serviços: bluetooth, perfis de energia e rede")" do_services
old_install && add "$(t "переезд со старого имени DARK-4RMY" "move from the old name DARK-4RMY" "migrar do nome antigo DARK-4RMY")" do_migrate
# the login screen is the last root step: it goes before the build (run_steps closes the root shell there)
on login && add "$(t "экран входа: greetd, вид и страховка" "login screen: greetd, look and safety net" "tela de login: greetd, aparência e rede de segurança")" do_greet
add "$(t "шелл: сборка из исходников" "shell: build from source" "shell: compilar do código-fonte")" do_build_shell
add "$(t "файлы: yami-files и диалог выбора файлов" "files: yami-files and the file dialog" "arquivos: yami-files e diálogo de arquivos")" do_files
add "$(t "шрифты: Tiny5, DotGothic16, Terminess, Monocraft" "fonts: Tiny5, DotGothic16, Terminess, Monocraft" "fontes: Tiny5, DotGothic16, Terminess e Monocraft")" do_fonts
add "$(t "niri: конфиг и настройки под железо" "niri: config and hardware settings" "niri: configuração e ajustes de hardware")" do_niri_config
add "$(t "палитра «$(pal_name "$pal")»: шелл, kitty, fastfetch, обои" "palette «$(pal_name "$pal")»: shell, kitty, fastfetch, wallpaper" "paleta «$(pal_name "$pal")»: shell, kitty, fastfetch e papel de parede")" do_dotfiles
on look && add "$(t "тема программ: Qt, KDE, GTK" "app theme: Qt, KDE, GTK" "tema dos aplicativos: Qt, KDE e GTK")" do_look
add "$(t "следы прошлых версий" "leftovers of earlier versions" "restos de versões anteriores")" do_legacy
old_install && add "$(t "убрать файлы под старым именем" "remove files under the old name" "remover arquivos com o nome antigo")" do_drop_old
add "$(t "запись об установке (для yami update)" "install record (for yami update)" "registro de instalação (para yami update)")" do_stamp
add "$(t "каких пакетов ещё не хватает" "packages still missing" "pacotes ainda ausentes")" do_need_pkgs
add "$(t "счётчик установок (один раз, без данных)" "install counter (once, no data)" "contador de instalações (uma vez, sem dados)")" do_count
((RUNNING_NIRI)) && add "$(t "перезапуск шелла" "restart the shell" "reiniciar o shell")" do_restart_shell

run_steps
# the root shell is already closed (run_steps, before the build); what follows asks for the
# password on its own, and only where it is needed
if [[ $SU == asroot ]]; then
  root_stop
  SU=$ROOT_TOOL
fi

if on login && ((SUDO && !DRY && !BUILD_FAILED)); then
  say "$(t "экран входа: ставлю yami-greet (пароль администратора ещё раз)" "login screen: installing yami-greet (the administrator password once more)" "tela de login: instalando o yami-greet (a senha de administrador mais uma vez)")"
  greet_bin || { FAILED+=("yami-greet"); warn "$(t "yami-greet не встал: до него вход будет текстовым; поставить потом:" "yami-greet is not installed: the login is text until then; install it later:" "o yami-greet não foi instalado: até lá o login será em texto; instale depois:") $ROOT/tools/greet-install.sh"; }
fi

if on fork && ((!DRY && !BUILD_FAILED)); then
  printf '\n  %s%s%s\n\n' "$T" "$(t "сборка yami-niri — 5–25 минут, вывод прямо здесь:" "building yami-niri takes 5–25 minutes, output below:" "compilar o yami-niri leva de 5 a 25 minutos; saída abaixo:")" "$R"
  fork_build || FAILED+=("yami-niri")
elif ((DRY)) && on fork; then
  printf '  %s·%s %s%s%s\n' "$A" "$R" "$T" "$(t "yami-niri: сборка и включение со следующего входа" "yami-niri: build and enable from the next login" "yami-niri: compilar e ativar a partir do próximo login")" "$R"
fi

# ── summary ─────────────────────────────────────────────────────────────────
printf '\n  %s────────────────────────────────────────%s\n' "$F" "$R"
if ((${#FAILED[@]})); then
  warn "$(t "не вышло:" "failed:" "falhas:") ${FAILED[*]}"
  note "$(t "подробности:" "details:" "detalhes:") $LOG"
else
  printf '  %s%s%s %s%s %s%s\n' "$OK" "$(t "готово." "done." "concluído.")" "$R" "$D" "$(t "лог:" "log:" "registro:")" "$LOG" "$R"
fi
if [[ -s $MISSING_F ]]; then
  warn "$(t "этих пакетов нет в репозиториях" "not in the repositories" "fora dos repositórios") ($OS_NAME): $(sort -u "$MISSING_F" | tr '\n' ' ')"
  note "$(t "шелл работает и без них; поставь аналоги вручную, если нужно" "the shell works without them; install equivalents by hand if needed" "o shell funciona sem eles; instale equivalentes manualmente, se necessário")"
fi
if [[ -s $STATE/packages-needed ]] && ((!DRY)); then
  warn "$(t "для выбранного не хватает пакетов — одна команда (или настройки → обновления):" "the chosen components miss packages; one command (or settings → updates):" "faltam pacotes para os componentes escolhidos; um comando (ou configurações → atualizações):")"
  note "$("$BIN/yami-shell" update need --print 2>/dev/null)"
fi
if [[ $FAM == nixos ]]; then
  printf '\n  %s%s%s\n' "$T" "$(t "NixOS: система — через flake (docs/PACKAGING.md):" "NixOS: the system goes through the flake (docs/PACKAGING.md):" "NixOS: o sistema vai pelo flake (docs/PACKAGING.md):")" "$R"
  note "inputs.yami.url = \"github:Trendorin/yami\";"
  note "imports = [ yami.nixosModules.default ];  programs.yami.enable = true;"
fi
niri_ok || ((DRY)) || on fork || warn "$(t "нужен niri $NIRI_MIN или новее — поставь его или запусти без --no-fork" "niri $NIRI_MIN or newer is required: install it or rerun without --no-fork" "niri $NIRI_MIN ou superior é necessário: instale ou execute novamente sem --no-fork")"

if ((RUNNING_NIRI)); then next="$(t "всё уже применилось; Qt и GTK-программы — после перезапуска" "everything is live; Qt and GTK apps pick it up after a restart" "tudo muda na hora; aplicativos Qt e GTK adotam as mudanças ao reiniciar")"
else next="$(t "в экране входа выбери сессию «Niri» (или niri-session из TTY)" "pick the «Niri» session at login (or run niri-session from a TTY)" "escolha a sessão «Niri» no login (ou execute niri-session num TTY)")"; fi
k() { printf '  %s%s%s %s%s%s %s%s%s %s%s%s\n' "$A" "$(pad "$1" 16)" "$R" "$D" "$(pad "$2" 22)" "$R" "$A" "$(pad "$3" 16)" "$R" "$D" "$4" "$R"; }
printf '\n  %s%s%s%s  %s%s%s\n\n' "$T" "$B" "$(t "дальше" "next" "próximo")" "$R" "$T" "$next" "$R"
k "Mod+Space" "$(t "программы" "launcher" "menu de aplicativos")" "Mod+V" "$(t "буфер обмена" "clipboard" "área de transferência")"
k "Mod+S" "$(t "настройки, палитра" "settings, palette" "configurações, paleta")" "Mod+Shift+Space" "$(t "стол / лента" "desk / tiling" "livre / mosaico")"
k "Mod+Escape" "$(t "блокировка" "lock" "bloquear")" "Mod+N" "$(t "свернуть окно" "minimize" "minimizar")"
k "Mod+Shift+E" "$(t "питание" "power" "energia")" "Mod+Shift+/" "$(t "все клавиши" "all keys" "todas as teclas")"
k "Mod+Shift+S" "$(t "скриншот" "screenshot" "captura de tela")" "Mod+E" "$(t "файлы" "files" "arquivos")"
printf '\n  %s%s%s\n' "$F" "$(t "клавиши меняются в настройках → клавиши; всё остальное: yami help, yami config list" "rebind keys in settings → keys; everything else: yami help, yami config list" "redefina os atalhos em configurações → atalhos; demais opções: yami help, yami config list")" "$R"
printf '  %s%s%s\n' "$F" "$(t "обновления: настройки → обновления или yami update" "updates: settings → updates or yami update" "atualizações: configurações → atualizações ou yami update")" "$R"
if on fork; then
  printf '  %s%s ~/.local/lib/yami-niri/yami-niri.sh status | off%s\n\n' "$F" "$(t "yami-niri:" "yami-niri:" "yami-niri:")" "$R"
else
  printf '  %s%s %s/fork/yami-niri.sh build%s\n\n' "$F" "$(t "наш niri для игр и плавности:" "our niri for games and smoothness:" "nosso niri para jogos e fluidez:")" "$ROOT" "$R"
fi

exit 0
}
