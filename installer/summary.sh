# The end of a run: what failed (with the log), packages a repository lacks, what the chosen components
# still miss (one command), and for a first install what comes next with the keys to start from.
# shellcheck shell=bash

# an update: one line, or what failed (exit 1)
summary_update() {
  if ((${#FAILED[@]})); then
    warn "$(t "не вышло:" "failed:" "falhas:") ${FAILED[*]}"
    note "$(t "подробности:" "details:" "detalhes:") $LOG"
    exit 1
  fi
  say "$(t "обновлено" "updated" "atualizado")"
  exit 0
}

summary_install() {
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

  local next
  if ((RUNNING_NIRI)); then next="$(t "всё уже применилось; Qt и GTK-программы — после перезапуска" "everything is live; Qt and GTK apps pick it up after a restart" "tudo muda na hora; aplicativos Qt e GTK adotam as mudanças ao reiniciar")"
  else next="$(t "в экране входа выбери сессию «Niri» (или niri-session из TTY)" "pick the «Niri» session at login (or run niri-session from a TTY)" "escolha a sessão «Niri» no login (ou execute niri-session num TTY)")"; fi
  printf '\n  %s%s%s%s  %s%s%s\n\n' "$T" "$B" "$(t "дальше" "next" "próximo")" "$R" "$T" "$next" "$R"
  key_row "Mod+Space" "$(t "программы" "launcher" "menu de aplicativos")" "Mod+V" "$(t "буфер обмена" "clipboard" "área de transferência")"
  key_row "Mod+S" "$(t "настройки, палитра" "settings, palette" "configurações, paleta")" "Mod+Shift+Space" "$(t "стол / лента" "desk / tiling" "livre / mosaico")"
  key_row "Mod+Escape" "$(t "блокировка" "lock" "bloquear")" "Mod+N" "$(t "свернуть окно" "minimize" "minimizar")"
  key_row "Mod+Shift+E" "$(t "питание" "power" "energia")" "Mod+Shift+/" "$(t "все клавиши" "all keys" "todas as teclas")"
  key_row "Mod+Shift+S" "$(t "скриншот" "screenshot" "captura de tela")" "Mod+E" "$(t "файлы" "files" "arquivos")"
  printf '\n  %s%s%s\n' "$F" "$(t "клавиши меняются в настройках → клавиши; всё остальное: yami help, yami config list" "rebind keys in settings → keys; everything else: yami help, yami config list" "redefina os atalhos em configurações → atalhos; demais opções: yami help, yami config list")" "$R"
  printf '  %s%s%s\n' "$F" "$(t "обновления: настройки → обновления или yami update" "updates: settings → updates or yami update" "atualizações: configurações → atualizações ou yami update")" "$R"
  if on fork; then
    printf '  %s%s ~/.local/lib/yami-niri/yami-niri.sh status | off%s\n\n' "$F" "yami-niri:" "$R"
  else
    printf '  %s%s %s/fork/yami-niri.sh build%s\n\n' "$F" "$(t "наш niri для игр и плавности:" "our niri for games and smoothness:" "nosso niri para jogos e fluidez:")" "$ROOT" "$R"
  fi
}
key_row() { printf '  %s%s%s %s%s%s %s%s%s %s%s%s\n' "$A" "$(pad "$1" 16)" "$R" "$D" "$(pad "$2" 22)" "$R" "$A" "$(pad "$3" 16)" "$R" "$D" "$4" "$R"; }
