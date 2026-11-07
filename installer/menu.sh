# The choices (palette, panels, theme, app font, fastfetch) and the interactive
# pages: the language, the summary first (enter installs the defaults, «s» only the shell), «c» walks
# through the pages (components, what to restyle, the look). Without a terminal or with --yes nothing
# is asked. An update keeps what is set.
# shellcheck shell=bash

# ── the choices; the palettes are the shell's own ──────────────────────────
PAL_ID=(blood ash sakura amber venom ice dusk)
PAL_RU=(кровь пепел сакура янтарь яд лёд сумрак)
PAL_EN=(blood ash sakura amber venom ice dusk)
PAL_PT=(sangue cinza sakura âmbar veneno gelo crepúsculo)
PAL_HEX=(c4161c a8a29c d9577c d98e1f 5fae34 3a9fd0 8a5cf0)
# panel look: glass (panel opacity), lift (lighter graphite), inner (cards and graphs), dim
# (behind menus); percent, written to the shell settings
LOOK_ID=(matte glass light solid)
LOOK_RU=(матовое стекло светлое плотное)
LOOK_EN=(matte glass light solid)
LOOK_PT=(fosco vidro claro sólido)
LOOK_GLASS=(78 55 85 100) LOOK_LIFT=(0 20 60 0) LOOK_INNER=(100 55 80 100) LOOK_DIM=(50 35 30 60)
# theme: dark graphite (default) or light paper; the same palette, the whole rice follows
THEME_ID=(dark light)
# app font: readable sans (default) or the pixel Tiny5 everywhere; the shell stays pixel
FONT_ID=(sans pixel)
# fastfetch: mini (default) or full with the hardware
FETCH_ID=(mini full)

# index of $2 in the array named $1, or nothing
idx_of() { local -n a=$1; local i; for i in "${!a[@]}"; do [[ ${a[$i]} == "$2" ]] && { echo "$i"; return 0; }; done; return 1; }

# flags first, then what the settings file already says (one read), else the defaults
choices_init() {
  pal=0 look=0 theme=0 font=0 fetch=0 LOOK_SET=0 THEME_SET=0 FETCH_SET=0
  local cur=() i
  if [[ -f $SET_JSON ]]; then
    # palette, glass lift inner (percent), light, app font, fastfetch
    read -r -a cur < <(python3 -I -c '
import json, sys
try:
    d = json.load(open(sys.argv[1]))
except (OSError, ValueError):
    d = {}
d = d if isinstance(d, dict) else {}
num = lambda k, v: round(float(d.get(k, v)) * 100) if isinstance(d.get(k, v), (int, float)) else round(v * 100)
# an older install without fontUi had the pixel font in apps
pixel = "fontUi" not in d or d.get("fontUi") == ""
print(d.get("palette") or "-", num("glass", 0.78), num("lift", 0), num("inner", 1), int(bool(d.get("light"))), int(pixel), int(bool(d.get("fetchFull"))))
' "$SET_JSON" 2>/dev/null)
  fi
  if [[ -n $PALETTE ]]; then
    pal="$(idx_of PAL_ID "$PALETTE")" || { echo "--palette: ${PAL_ID[*]}" >&2; exit 2; }
  elif ((${#cur[@]} == 7)); then
    pal="$(idx_of PAL_ID "${cur[0]}")" || pal=0
  fi
  if [[ -n $LOOK_OPT ]]; then
    look="$(idx_of LOOK_ID "$LOOK_OPT")" || { echo "--look: ${LOOK_ID[*]}" >&2; exit 2; }
    LOOK_SET=1
  elif ((${#cur[@]} == 7)); then
    for i in "${!LOOK_ID[@]}"; do [[ "${LOOK_GLASS[$i]} ${LOOK_LIFT[$i]} ${LOOK_INNER[$i]}" == "${cur[1]} ${cur[2]} ${cur[3]}" ]] && look=$i; done
  fi
  if [[ -n $THEME_OPT ]]; then
    theme="$(idx_of THEME_ID "$THEME_OPT")" || { echo "--theme: dark or light" >&2; exit 2; }
    THEME_SET=1
  elif ((${#cur[@]} == 7)); then
    theme=${cur[4]}
  fi
  if [[ -n $FONT_OPT ]]; then
    font="$(idx_of FONT_ID "$FONT_OPT")" || { echo "--font: sans or pixel" >&2; exit 2; }
  elif ((${#cur[@]} == 7)); then
    font=${cur[5]}
  fi
  if [[ -n $FETCH_OPT ]]; then
    fetch="$(idx_of FETCH_ID "$FETCH_OPT")" || { echo "--fetch: mini or full" >&2; exit 2; }
    FETCH_SET=1
  elif ((${#cur[@]} == 7)); then
    fetch=${cur[6]}
  fi
  return 0
}

theme_name() { case ${THEME_ID[$1]} in dark) t "тёмная" "dark" "escuro" ;; light) t "светлая" "light" "claro" ;; esac; }
theme_desc() {
  case ${THEME_ID[$1]} in
    dark) t "графит и акцент — как задумано" "graphite and the accent: the original look" "grafite e destaque: a aparência original" ;;
    light) t "светлая бумага, акцент темнее; GTK, Qt, kitty и курсор — тоже" "light paper, a deeper accent; GTK, Qt, kitty and the cursor follow" "papel claro, destaque mais escuro; GTK, Qt, kitty e o cursor acompanham" ;;
  esac
}
fetch_name() { case ${FETCH_ID[$1]} in mini) t "короткий" "mini" "resumido" ;; full) t "расширенный" "extended" "completo" ;; esac; }
fetch_desc() {
  case ${FETCH_ID[$1]} in
    mini) t "кто, система, окружение — без железа" "who, system, desktop: no hardware" "usuário, sistema e desktop: sem hardware" ;;
    full) t "ещё ядро, пакеты, процессор, видеокарта, память, диск, экран" "also kernel, packages, CPU, GPU, memory, disk, display" "também kernel, pacotes, CPU, GPU, memória, disco e tela" ;;
  esac
}
font_name() { case ${FONT_ID[$1]} in sans) t "обычный (Noto Sans)" "regular (Noto Sans)" "normal (Noto Sans)" ;; pixel) t "пиксельный (Tiny5)" "pixel (Tiny5)" "pixel (Tiny5)" ;; esac; }
font_desc() {
  case ${FONT_ID[$1]} in
    sans) t "программы читаются как обычно; шелл, файлы и экран входа — пиксельные" "apps read as usual; the shell, files and login screen stay pixel" "aplicativos com leitura normal; shell, arquivos e tela de login continuam em pixel" ;;
    pixel) t "пиксельный шрифт во всех программах, ровно по сетке экрана" "the pixel font in every app, exactly on the screen's grid" "fonte em pixel em todos os aplicativos, alinhada à grade da tela" ;;
  esac
}

look_name() { case $L in ru) printf '%s' "${LOOK_RU[$1]}" ;; pt-BR) printf '%s' "${LOOK_PT[$1]}" ;; *) printf '%s' "${LOOK_EN[$1]}" ;; esac; }
look_desc() {
  case ${LOOK_ID[$1]} in
    matte) t "тёмный графит, панели почти сплошные — как задумано" "dark graphite, nearly solid panels: the original look" "grafite escuro, painéis quase sólidos: a aparência original" ;;
    glass) t "панели и карточки прозрачные, стол просвечивает" "translucent panels and cards, the desktop shows through" "painéis e cartões translúcidos; a área de trabalho aparece ao fundo" ;;
    light) t "графит светлее, без глухого чёрного" "lighter graphite, no deep black" "grafite mais claro, sem preto intenso" ;;
    solid) t "всё сплошное, сильное затемнение под меню" "fully solid, strong dim behind menus" "totalmente sólido, escurecimento forte atrás dos menus" ;;
  esac
}

pal_name() { case $L in ru) printf '%s' "${PAL_RU[$1]}" ;; pt-BR) printf '%s' "${PAL_PT[$1]}" ;; *) printf '%s' "${PAL_EN[$1]}" ;; esac; }

pick_lang() {
  local cur=1 opts=("Русский" "English" "Português (Brasil)") codes=(ru en pt-BR) i mark style
  case $L in ru) cur=0 ;; pt-BR) cur=2 ;; esac
  printf '\e[?25l'
  while :; do
    printf '\e[H\e[J'
    banner 0
    printf '  %s%slanguage · язык · idioma%s   %s↑↓ · enter%s\n\n' "$T" "$B" "$R" "$F" "$R"
    for i in "${!opts[@]}"; do
      if ((i == cur)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s\n' "$mark" "$style" "${opts[$i]}" "$R"
    done
    case "$(key)" in
      up | k | left) cur=$(((cur + ${#opts[@]} - 1) % ${#opts[@]})) ;;
      down | j | right) cur=$(((cur + 1) % ${#opts[@]})) ;;
      enter) break ;;
      q | esc | eof) bye ;;
    esac
  done
  L="${codes[$cur]}"
}

# --shell-only and «s»: niri and the shell; nothing else of ours, nothing of anyone else's restyled
shell_only() {
  local i
  for i in "${!C_ID[@]}"; do [[ ${C_ID[$i]} == core ]] || C_ON[i]=0; done
  t_all 0
  look_sync
}

pick_components() {
  local rows=() cur=0 n i k id box mark style lock
  for id in "${C_ID[@]}"; do c_shown "$id" && rows+=("$id"); done
  n=${#rows[@]}
  printf '\e[?25l'
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "что ставим" "components" "componentes")" "$R" "$F" \
      "$(t "↑↓ · пробел — галочка · a — всё · s — только шелл · enter — дальше · q — выход" "↑↓ · space toggle · a all · s shell only · enter next · q quit" "↑↓ · espaço marcar · a tudo · s só o shell · enter avançar · q sair")" "$R"
    for ((i = 0; i < n; i++)); do
      id=${rows[$i]} k=$(c_idx "${rows[$i]}")
      if ((C_ON[k])); then box="${A}[■]${R}"; else box="${F}[ ]${R}"; fi
      if ((i == cur)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      lock=""
      can "$id" || lock=" $F($(t "нужен root и пакетный менеджер" "needs root and a package manager" "requer root e um gerenciador de pacotes"))$R"
      printf '  %s %s %s%s%s%s\n' "$mark" "$box" "$style" "$(c_name "$id")" "$R" "$lock"
    done
    printf '\n  %s%s%s\n' "$D" "$(c_desc "${rows[$cur]}")" "$R"
    printf '  %s%s%s\n' "$F" "$(t "всегда: шелл (сборка из исходников), его шрифты, конфиг niri; чужие конфиги — на следующей странице" \
      "always: the shell (built from source), its fonts, niri's config; other programs' configs: the next page" "sempre: o shell (compilado do código-fonte), suas fontes, a configuração do niri; configurações de outros programas: próxima página")" "$R"
    printf '  %s%s: %s%s%s\n' "$F" "$(t "система" "system" "sistema")" "$D" "$OS_NAME${PM:+ · $PM}" "$R"
    case "$(key)" in
      up | k) cur=$(((cur + n - 1) % n)) ;;
      down | j) cur=$(((cur + 1) % n)) ;;
      space) k=$(c_idx "${rows[$cur]}"); can "${rows[$cur]}" && C_ON[k]=$((1 - C_ON[k])) ;;
      # «all» without the initramfs: that one is only ever chosen on its own
      a | "ф") for id in "${rows[@]}"; do [[ $id != nvboot ]] && can "$id" && c_set "$id" 1; done ;;
      s | "ы") shell_only ;;
      enter) return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

preview_bar() { # how the bar looks in this palette
  local bg=$'\e[48;2;14;12;12m' h=${PAL_HEX[$pal]} on=$'\e[38;2;255;255;255m'
  printf '  %s%s %s%s 1 %s%s %s2 3 %s│ %s■%s %s  %s%s %s■■■■%s■■■■%s 34%%  %s│  %s♪ ▂▅▇▃▆▂  %s17:42 %s\n' \
    "$bg" "$T" "$AB" "$on" "$R" "$bg" "$D" "$F" "$A" "$T" "$(t "стол" "desk" "livre")" "$D" "$(t "цп" "cpu" "cpu")" "$A" "$F" "$D" "$F" "$A" "$T" "$R"
  printf '  '
  local k
  for k in -80 -65 -45 -25 0 25 45 65; do printf '\e[48;2;%sm    ' "$(shade "$h" "$k")"; done
  printf '%s\n' "$R"
}

pick_palette() {
  local n=${#PAL_ID[@]} i mark style
  while :; do
    paint
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "палитра" "palette" "paleta")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом меняется в настройках (Mod+S)" "↑↓ move · enter next · change it later in settings (Mod+S)" "↑↓ escolher · enter avançar · mude depois nas configurações (Mod+S)")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == pal)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s \e[48;2;%sm      %s  %s%s%s %s#%s%s\n' "$mark" "$(rgb "${PAL_HEX[$i]}")" "$R" "$style" "$(pad "$(pal_name "$i")" 8)" "$R" "$F" "${PAL_HEX[$i]}" "$R"
    done
    printf '\n'
    preview_bar
    case "$(key)" in
      up | k) pal=$(((pal + n - 1) % n)) ;;
      down | j) pal=$(((pal + 1) % n)) ;;
      enter) return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

# hex a → hex b by k percent, as hex
mixh() {
  local o="" c x y
  for c in 0 2 4; do
    x=$((0x${1:c:2})) y=$((0x${2:c:2}))
    o+="$(printf '%02x' $((x + (y - x) * $3 / 100)))"
  done
  printf '%s' "$o"
}
# a small panel over a striped "desktop": how dark, see-through and lifted it is
preview_look() {
  local i=$1 acc=${PAL_HEX[$pal]} wall1 wall2 light panel card bar row c j
  wall1="$(mixh 1a1416 "$acc" 30)" wall2="$(mixh 0c0a0a "$acc" 12)"
  light="$(mixh 4a4242 "$acc" 12)"
  panel="$(mixh 0e0c0c "$light" $((LOOK_LIFT[i] * 55 / 100)))"
  card="$(mixh 161313 "$light" $((LOOK_LIFT[i] * 55 / 100)))"
  for row in 0 1 2 3 4 5; do
    printf '  '
    for ((j = 0; j < 44; j++)); do
      c=$wall2
      (((j + row) % 6 < 3)) && c=$wall1
      if ((row >= 1 && row <= 4 && j >= 4 && j < 40)); then
        c="$(mixh "$c" "$panel" "${LOOK_GLASS[i]}")"
        if ((row >= 2 && row <= 3 && j >= 7 && j < 37)); then
          c="$(mixh "$c" "$card" "${LOOK_INNER[i]}")"
          ((row == 3 && j >= 9 && j < 27)) && c=$acc
        fi
        ((row == 1 && j >= 6 && j < 8)) && c=$acc
      fi
      printf '\e[48;2;%sm ' "$(rgb "$c")"
    done
    printf '%s\n' "$R"
  done
}

pick_look() {
  local n=${#LOOK_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "панели" "panels" "painéis")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · тонко — в настройках → цвет" "↑↓ move · enter next · fine-tune in settings → color" "↑↓ escolher · enter avançar · ajuste fino em configurações → cores")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == look)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(look_name "$i")" 10)" "$R" "$F" "$(look_desc "$i")" "$R"
    done
    printf '\n'
    preview_look "$look"
    case "$(key)" in
      up | k) look=$(((look + n - 1) % n)) ;;
      down | j) look=$(((look + 1) % n)) ;;
      enter) LOOK_SET=1; return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

pick_font() {
  local n=${#FONT_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "шрифт программ" "app font" "fonte dos aplicativos")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом: настройки → шрифты" "↑↓ move · enter next · later: settings → fonts" "↑↓ escolher · enter avançar · depois: configurações → fontes")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == font)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(font_name "$i")" 22)" "$R" "$F" "$(font_desc "$i")" "$R"
    done
    case "$(key)" in
      up | k | down | j) font=$((1 - font)) ;;
      enter) return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

pick_theme() {
  local n=${#THEME_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "$(t "тема" "theme" "tema")" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом: настройки → цвет" "↑↓ move · enter next · later: settings → color" "↑↓ escolher · enter avançar · depois: configurações → cores")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == theme)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(theme_name "$i")" 22)" "$R" "$F" "$(theme_desc "$i")" "$R"
    done
    case "$(key)" in
      up | k | down | j) theme=$((1 - theme)) ;;
      enter) THEME_SET=1; return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

pick_fetch() {
  local n=${#FETCH_ID[@]} i mark style
  while :; do
    screen
    printf '  %s%s%s%s   %s%s%s\n\n' "$T" "$B" "fastfetch" "$R" "$F" \
      "$(t "↑↓ выбор · enter — дальше · потом: настройки → вид" "↑↓ move · enter next · later: settings → look" "↑↓ escolher · enter avançar · depois: configurações → visual")" "$R"
    for ((i = 0; i < n; i++)); do
      if ((i == fetch)); then mark="$A▸$R" style="$T$B"; else mark=" " style="$D"; fi
      printf '  %s %s%s%s %s%s%s\n' "$mark" "$style" "$(pad "$(fetch_name "$i")" 22)" "$R" "$F" "$(fetch_desc "$i")" "$R"
    done
    case "$(key)" in
      up | k | down | j) fetch=$((1 - fetch)) ;;
      enter) FETCH_SET=1; return 0 ;;
      q | esc | eof) bye ;;
    esac
  done
}

confirm() {
  screen
  printf '  %s%s%s%s\n\n' "$T" "$B" "$(t "сводка" "summary" "resumo")" "$R"
  local i
  printf '  %s■%s %s%s%s %s(%s)%s\n' "$A" "$R" "$T" "$OS_NAME" "$R" "$F" "${PM:-$(t "без пакетов" "no packages" "sem pacotes")}" "$R"
  printf '  %s■%s %s%s%s\n' "$A" "$R" "$T" "$(t "шелл, его шрифты, конфиг niri" "the shell, its fonts, niri's config" "o shell, suas fontes, a configuração do niri")" "$R"
  for i in "${!C_ID[@]}"; do
    [[ ${C_ID[$i]} == look ]] && continue
    ((C_ON[i])) && printf '  %s■%s %s%s%s\n' "$A" "$R" "$T" "$(c_name "${C_ID[$i]}")" "$R"
  done
  local rs="" id
  for id in "${T_ID[@]}"; do t_on "$id" && rs+="${rs:+, }$(t_name "$id")"; done
  if [[ -n $rs ]]; then
    printf '  %s■%s %s%s: %s%s\n' "$A" "$R" "$T" "$(t "перекрасить" "restyle" "recolorir")" "$rs" "$R"
  else
    printf '  %s■%s %s%s%s\n' "$A" "$R" "$T" "$(t "чужие программы не трогаю" "other programs left alone" "outros programas ficam intocados")" "$R"
  fi
  printf '  %s■%s %s%s: %s%s \e[48;2;%sm    %s\n' "$A" "$R" "$T" "$(t "палитра" "palette" "paleta")" "$(pal_name "$pal")" "$R" "$(rgb "${PAL_HEX[$pal]}")" "$R"
  ((LOOK_SET)) && printf '  %s■%s %s%s: %s%s\n' "$A" "$R" "$T" "$(t "панели" "panels" "painéis")" "$(look_name "$look")" "$R"
  printf '  %s■%s %s%s: %s%s\n' "$A" "$R" "$T" "$(t "тема" "theme" "tema")" "$(theme_name "$theme")" "$R"
  t_any gtk qt && printf '  %s■%s %s%s: %s%s\n' "$A" "$R" "$T" "$(t "шрифт программ" "app font" "fonte dos aplicativos")" "$(font_name "$font")" "$R"
  t_on fastfetch && printf '  %s■%s %sfastfetch: %s%s\n' "$A" "$R" "$T" "$(fetch_name "$fetch")" "$R"
  printf '  %s■%s %s%s%s\n\n' "$A" "$R" "$T" "$(t "язык: русский" "language: English" "idioma: português do Brasil")" "$R"
  [[ $FAM == nixos ]] && note "$(t "NixOS: так ставится только домашняя папка; вся система — flake.nix (docs/PACKAGING.md)" "NixOS: this installs the home folder only; the whole system: flake.nix (docs/PACKAGING.md)" "NixOS: isto instala só a pasta pessoal; o sistema inteiro: flake.nix (docs/PACKAGING.md)")"
  ((ATOMIC)) && note "$(t "атомарная Fedora: пакеты ставятся слоем rpm-ostree, нужна перезагрузка" "atomic Fedora: packages are layered with rpm-ostree, a reboot is needed" "Fedora atômico: os pacotes são adicionados com rpm-ostree; é necessário reiniciar")"
  ((RUNNING_NIRI)) && note "$(t "niri уже запущен — шелл перезапустится сам, остальное подхватится на лету" "niri is running: the shell restarts by itself, the rest reloads live" "niri está em execução: o shell reinicia sozinho; o restante recarrega na hora")"
  ((DRY)) && note "$(t "--dry-run: только план, ничего не меняется" "--dry-run: plan only, nothing changes" "--dry-run: apenas o plano, nada será alterado")"
  printf '\n  %s%s%s %s[%s]%s ' "$T" "$(t "поехали?" "go?" "vamos?")" "$R" "$F" "$(t "enter — ставить, c — выбрать иначе, s — только шелл, q — выход" "enter install, c customize, s shell only, q quit" "enter instalar, c personalizar, s só o shell, q sair")" "$R"
  printf '\e[?25h'
  while :; do
    case "$(key)" in
      enter | y) printf '\n'; return 0 ;;
      c | e | "с") return 1 ;;
      s | "ы") return 2 ;;
      q | esc | eof) bye ;;
    esac
  done
}
# the summary first: enter installs what is shown; «c» goes through every page and back here, «s»
# leaves only the shell and shows the summary again
ask_all() {
  ((LANG_ASK)) && pick_lang
  local r chg=0
  while :; do
    confirm
    r=$?
    case $r in
      0) break ;;
      2) shell_only; chg=1 ;;
      *)
        chg=1
        pick_components
        pick_restyle
        pick_palette
        pick_look
        pick_theme
        # the app font is GTK's and Qt's, the fastfetch look is fastfetch's
        t_any gtk qt && pick_font
        t_on fastfetch && pick_fetch ;;
    esac
  done
  # the choice as made (components_limit already switched off what this run cannot do)
  ((chg)) && C_WANT=("${C_ON[@]}")
  return 0
}
