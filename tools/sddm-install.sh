#!/usr/bin/env bash
# Install (or update) the yami SDDM theme on this machine and make SDDM the login manager from
# the next boot. Asks for sudo. Run from the repo: tools/sddm-install.sh [--test] [--no-enable]
#   --test       after installing, open the greeter in a window (sddm-greeter-qt6 --test-mode)
#   --no-enable  install the theme only, do not touch display-manager.service
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST=0 ENABLE=1
for a; do case "$a" in --test) TEST=1 ;; --no-enable) ENABLE=0 ;; *) echo "unknown: $a" >&2; exit 2 ;; esac; done
DST=/usr/share/sddm/themes/yami
if ! command -v sddm >/dev/null; then
  if command -v pacman >/dev/null; then sudo pacman -S --needed --noconfirm sddm
  else echo "install sddm with your package manager first" >&2; exit 1; fi
fi
SRC="$(mktemp -d)/yami"
"$ROOT/tools/sddm-build.sh" "$SRC" >/dev/null
sudo rm -rf -- "$DST"
sudo mkdir -p "$(dirname "$DST")"
sudo cp -r "$SRC" "$DST"
# the palette and wallpaper follow the shell: these two files stay yours
sudo touch "$DST/theme.conf.user" "$DST/background.png"
sudo chown "$(id -u):$(id -g)" "$DST/theme.conf.user" "$DST/background.png"
sudo chmod 644 "$DST/theme.conf.user" "$DST/background.png"
python3 -I "$ROOT/tools/theme.py" --sddm "$DST"
sudo mkdir -p /etc/sddm.conf.d
# the pixel cursor on the login screen too: the greeter reads /usr/share/icons only
CUR="$(mktemp -d)/yami-cursor"
python3 -I "$ROOT/tools/theme.py" --cursor-to "$CUR"
sudo rm -rf -- /usr/share/icons/yami-cursor
sudo cp -r "$CUR" /usr/share/icons/yami-cursor
printf '%s\n' "# yami: login screen theme" "[Theme]" "Current=yami" "CursorTheme=yami-cursor" "CursorSize=24" \
  "" "[Users]" "RememberLastUser=true" "RememberLastSession=true" | sudo tee /etc/sddm.conf.d/10-yami.conf >/dev/null
# plymouth: never let a hung boot splash block multi-user.target (see system/plymouth)
if [[ -f /usr/lib/systemd/system/plymouth-quit.service ]]; then
  for u in plymouth-quit plymouth-quit-wait; do
    sudo install -Dm644 "$ROOT/system/plymouth/10-yami.conf" "/etc/systemd/system/$u.service.d/10-yami.conf"
  done
  sudo systemctl daemon-reload
fi
if ((ENABLE)) && [[ -d /run/systemd/system ]]; then
  sudo systemctl enable -f sddm.service
  echo "sddm: enabled from the next boot (the current session is untouched)"
fi
echo "theme: $DST"
if ((TEST)); then
  G="$(command -v sddm-greeter-qt6 || command -v sddm-greeter)"
  "$G" --test-mode --theme "$DST"
fi
