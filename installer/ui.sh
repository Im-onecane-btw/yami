# What the installer looks like in the terminal. Colours from the palette, the
# message helpers (say, note, warn), the animated banner, keys for the menus and the step runner:
# a spinner and a progress bar, the log, «not needed» (exit 99) and failures with the log's tail.
# Sourced by install.sh; defines only (install.sh calls paint and ui_traps).
# shellcheck shell=bash

# t "русский" "english": the text in the chosen language
t() { if [[ $L == ru ]]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }

# ── paint ───────────────────────────────────────────────────────────────────
rgb() { printf '%d;%d;%d' "0x${1:0:2}" "0x${1:2:2}" "0x${1:4:2}"; }
# accent mixed towards white (k>0) or black (k<0), in percent
shade() {
  local h=$1 k=$2 c out="" v
  for c in 0 2 4; do
    v=$((0x${h:c:2}))
    if ((k > 0)); then v=$((v + (255 - v) * k / 100)); else v=$((v * (100 + k) / 100)); fi
    out+="$v;"
  done
  printf '%s' "${out%;}"
}
paint() {
  [[ -t 1 ]] || { A="" AB="" AH="" AD=""; return; }
  local h=${PAL_HEX[$pal]}
  A=$'\e'"[38;2;$(rgb "$h")m"
  AB=$'\e'"[48;2;$(rgb "$h")m"
  AH=$'\e'"[38;2;$(shade "$h" 55)m"
  AD=$'\e'"[38;2;$(shade "$h" -55)m"
}
T=$'\e[38;2;235;229;228m' D=$'\e[38;2;150;140;140m' F=$'\e[38;2;88;78;78m'
OK=$'\e[38;2;120;200;120m' WARN=$'\e[38;2;255;170;40m' BAD=$'\e[38;2;255;74;61m'
B=$'\e[1m' R=$'\e[0m'
[[ -t 1 ]] || T="" D="" F="" OK="" WARN="" BAD="" B="" R=""

pad() { printf '%s%*s' "$1" $(($2 - ${#1} > 0 ? $2 - ${#1} : 0)) ''; }
say() { printf "  %s▪%s %s%s%s\n" "$A" "$R" "$T" "$*" "$R"; }
note() { printf "    %s%s%s\n" "$F" "$*" "$R"; }
warn() { printf "  %s!%s %s%s%s\n" "$WARN" "$R" "$T" "$*" "$R"; }
nap() { ((ANIM)) && sleep "$1"; return 0; }

cleanup() { printf '\e[?25h%s' "$R"; }
ui_traps() {
  trap cleanup EXIT
  trap 'cleanup; echo; exit 130' INT
  trap 'cleanup; exit 143' TERM
  # the terminal may die mid-install (kitty reloads fonts and themes): finish the steps anyway
  trap '' HUP
}

# ── banner: the logo dissolves in on a Bayer grid, then a light sweep ───────
LOGO1="▀▄▀ ▄▀█ █▀▄▀█ █"
LOGO2=" █  █▀█ █ ▀ █ █"
BAYER=(0 8 2 10 12 4 14 6 3 11 1 9 15 7 13 5)
logo_row() { # row text frame sweep
  local s=$2 f=$3 sw=$4 out="" i ch th
  for ((i = 0; i < ${#s}; i++)); do
    ch=${s:i:1}
    th=${BAYER[($1 % 4) * 4 + i % 4]}
    if [[ $ch == " " ]]; then out+=" "
    elif ((th < f - 2)); then
      if ((sw >= 0 && (i == sw || i == sw - 1))); then out+="$T$B$ch$R"
      elif ((sw >= 0 && i == sw - 2)); then out+="$AH$ch"
      else out+="$A$ch"; fi
    elif ((th < f)); then out+="$AD░"
    else out+=" "; fi
  done
  printf '  %s%s\e[K\n' "$out" "$R"
}
SUBTITLE=""
banner() { # 1 — animate
  SUBTITLE="$(t "пиксельный райс для niri" "pixel rice for niri")"
  local f sw
  printf '\n'
  if (($1)) && ((ANIM)); then
    printf '\e[?25l'
    for ((f = 0; f <= 18; f++)); do
      logo_row 0 "$LOGO1" "$f" -1
      logo_row 1 "$LOGO2" "$f" -1
      printf '\e[2A'
      sleep 0.022
    done
    for ((sw = 0; sw <= ${#LOGO1} + 3; sw += 2)); do
      logo_row 0 "$LOGO1" 99 "$sw"
      logo_row 1 "$LOGO2" 99 "$sw"
      printf '\e[2A'
      sleep 0.012
    done
  fi
  logo_row 0 "$LOGO1" 99 -1
  logo_row 1 "$LOGO2" 99 -1
  if (($1)) && ((ANIM)); then
    local line="────────────────────────────────────────" i
    printf '  %s' "$F"
    for ((i = 0; i < ${#line}; i += 4)); do printf '%s' "${line:i:4}"; sleep 0.008; done
    printf '%s\n  %s' "$R" "$D"
    for ((i = 0; i < ${#SUBTITLE}; i++)); do printf '%s' "${SUBTITLE:i:1}"; sleep 0.012; done
    printf '%s  %s// %s%s\n\n' "$R" "$F" "$(t "установка" "installer")" "$R"
  else
    printf '  %s────────────────────────────────────────%s\n' "$F" "$R"
    printf '  %s%s%s  %s// %s%s\n\n' "$D" "$SUBTITLE" "$R" "$F" "$(t "установка" "installer")" "$R"
  fi
}

# ── keys ────────────────────────────────────────────────────────────────────
key() {
  local k rest more
  # терминал закрыт (ssh оборвался, TTY ушёл): не крутиться в меню, а выйти
  IFS= read -rsn1 k || { echo eof; return; }
  if [[ $k == $'\e' ]]; then
    # по ssh и на медленном TTY хвост последовательности приходит позже: ждём дольше и
    # дочитываем всё (Home/End/F-клавиши длиннее двух байт), чтобы остаток не стал «клавишами»
    read -rsn2 -t 0.15 rest
    while IFS= read -rsn1 -t 0.01 more; do rest+=$more; done
    case "$rest" in "[A" | "OA") echo up ;; "[B" | "OB") echo down ;; "[C" | "OC") echo right ;; "[D" | "OD") echo left ;; "") echo esc ;; *) echo other ;; esac
  elif [[ $k == "" ]]; then echo enter
  elif [[ $k == " " ]]; then echo space
  else echo "$k"; fi
}
bye() { printf '\e[H\e[J  %s\n' "$(t "ну и ладно. ничего не тронуто." "ok, nothing was touched.")"; exit 0; }
screen() { printf '\e[H\e[J'; banner 0; }

# ── steps: spinner, elapsed time and a shimmering progress bar ──────────────
STEPS_T=() STEPS_F=()
add() { local title=$1; shift; STEPS_T+=("$title"); STEPS_F+=("$(printf '%q ' "$@")"); }
FAILED=() SKIPPED=() BUILD_FAILED=0
SPIN=(⣾ ⣽ ⣻ ⢿ ⡿ ⣟ ⣯ ⣷)
progress() { # done total frame
  local w=32 fill=$(($1 * 32 / $2)) s="" j hl=$(($3 % 44 - 6))
  for ((j = 0; j < w; j++)); do
    if ((j < fill)); then
      if ((j == hl || j == hl + 1)); then s+="$T█"; elif ((j == hl - 1 || j == hl + 2)); then s+="$AH█"; else s+="$A█"; fi
    elif ((j == fill)); then s+="$AD▓"
    else s+="$F░"; fi
  done
  printf '  %s%s %s%d/%d%s' "$s" "$R" "$D" "$1" "$2" "$R"
}
elapsed() { local s=$1; ((s >= 60)) && printf '%dm%02ds' $((s / 60)) $((s % 60)) || printf '%ds' "$s"; }
step() { # index
  local i=$1 title=${STEPS_T[$1]} n=${#STEPS_T[@]} t0=$SECONDS rc f=0
  printf '\n### %s\n' "$title" >>"$LOG"
  if ((DRY)); then
    local cmd=${STEPS_F[$i]} room=$(($(tput cols 2>/dev/null || echo 100) - ${#title} - 8))
    ((${#cmd} > room)) && cmd="${cmd:0:room-1}…"
    printf '  %s·%s %s%s%s %s%s%s\n' "$A" "$R" "$T" "$title" "$R" "$F" "$cmd" "$R"
    return
  fi
  if [[ ! -t 1 || $SU == sudo || $SU == doas ]]; then
    # no terminal, or root without the shared root shell (its prompts must be seen): run in the
    # foreground; [k/n]: the shell's updates tab draws its progress from it
    printf '  %s▸%s %s[%d/%d] %s%s\n' "$A" "$R" "$T" $((i + 1)) "$n" "$title" "$R"
    eval "${STEPS_F[$i]}" 2>&1 | tee -a "$LOG" | sed 's/^/      /'
    rc=${PIPESTATUS[0]}
  else
    # root commands go to the root shell authorised before the steps (root_start): nothing here
    # asks for a password, so every step can run behind the spinner
    (eval "${STEPS_F[$i]}") >>"$LOG" 2>&1 </dev/null &
    local pid=$!
    printf '\e[?25l'
    while kill -0 "$pid" 2>/dev/null; do
      printf '\r\e[K  %s%s%s %s%s%s %s%s%s\n\e[K' "$A" "${SPIN[f % 8]}" "$R" "$T" "$title" "$R" "$F" "$(elapsed $((SECONDS - t0)))" "$R"
      progress "$i" "$n" "$f"
      printf '\e[1A\r'
      ((f++))
      sleep 0.07
    done
    wait "$pid"
    rc=$?
  fi
  local dt
  dt="$(elapsed $((SECONDS - t0)))"
  if ((rc == 0)); then
    printf '\r\e[K  %s✓%s %s%s%s %s%s%s\n\e[K' "$OK" "$R" "$T" "$title" "$R" "$F" "$dt" "$R"
  elif ((rc == 99)); then
    printf '\r\e[K  %s–%s %s%s%s %s%s%s\n\e[K' "$F" "$R" "$D" "$title" "$R" "$F" "$(t "не нужно" "not needed")" "$R"
    SKIPPED+=("$title")
  else
    printf '\r\e[K  %s✗%s %s%s%s %s— %s%s\n\e[K' "$BAD" "$R" "$T" "$title" "$R" "$F" "$(t "хвост лога:" "log tail:")" "$R"
    tail -n 6 "$LOG" | sed "s/^/      $F/; s/\$/$R/"
    FAILED+=("$title")
  fi
  printf '\e[?25h'
}
run_steps() {
  local i nf
  for ((i = 0; i < ${#STEPS_T[@]}; i++)); do
    nf=${#FAILED[@]}
    # every root step comes before the build: the root shell is closed here, so no build script of
    # a crate (minutes of code from crates.io) runs while its pipes are open (see root_start). A step
    # after this point that still asked for root would fail (asroot: «the root shell is gone»)
    [[ ${STEPS_F[$i]%% *} == do_build_shell ]] && root_stop
    step "$i"
    # a build that failed stops here. An update: the old binaries stay with the configs they match,
    # and the install record keeps the old version, so the update is offered again. A first
    # install: no niri config that starts a shell which is not there (the packages are in, a rerun
    # continues from the build)
    if [[ ${STEPS_F[$i]%% *} == do_build_shell ]] && ((${#FAILED[@]} > nf)); then
      BUILD_FAILED=1
      if ((UPDATE)); then
        warn "$(t "сборка не удалась — обновление остановлено, стоит прежняя версия" "the build failed: update stopped, the previous version stays")"
      else
        warn "$(t "шелл не собрался — конфиги не тронуты; поправь причину (лог ниже) и запусти установку снова" "the shell did not build: your configs are untouched; fix the cause (log below) and run the installer again")"
      fi
      break
    fi
  done
  if [[ -t 1 ]] && ((!DRY)); then
    local f
    for ((f = 0; f < 22; f++)); do printf '\r'; progress "${#STEPS_T[@]}" "${#STEPS_T[@]}" $((f * 2)); nap 0.02; done
    printf '\n'
  fi
}

backup() { [[ -e "$1" && ! -L "$1" ]] && mv -- "$1" "$1.bak-$STAMP"; return 0; }
