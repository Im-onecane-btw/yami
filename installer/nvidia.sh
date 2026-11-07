# NVIDIA (the proprietary driver only): one file and nothing else of the driver. niri's own fix for
# its VRAM use (the niri wiki, «Nvidia»): an application profile that applies to the niri process
# alone. No environment for the session, no modprobe options, no initramfs. Versions up to 2.6 could
# put the driver into the initramfs (/etc/modprobe.d, mkinitcpio.conf.d or dracut.conf.d): those
# files stay where they are, `yami doctor` names them, --uninstall takes them out.
# shellcheck shell=bash

NV_PROFILE=/etc/nvidia/nvidia-application-profiles-rc.d/50-niri-vram.json
NV_OLD=(/etc/modprobe.d/yami-nvidia-kms.conf /etc/mkinitcpio.conf.d/90-yami-nvidia.conf /etc/dracut.conf.d/90-yami-nvidia.conf)

plan_nvidia() {
  on nvidia && add "$(t "NVIDIA: профиль niri против роста VRAM" "NVIDIA: niri's profile against VRAM growth" "NVIDIA: perfil do niri contra o crescimento de VRAM")" do_nvidia_profile
  return 0
}
do_nvidia_profile() { $SU install -Dm644 "$ROOT/system/nvidia/50-niri-vram.json" "$NV_PROFILE"; }

nvidia_old() { local f; for f in "${NV_OLD[@]}"; do [[ -e $f ]] && return 0; done; return 1; }
# --uninstall: the profile goes, and the driver in the initramfs from ≤ 2.6 (the boot images are
# rebuilt without it)
nvidia_uninstall() {
  [[ -e $NV_PROFILE ]] || nvidia_old || return 0
  say "$(t "NVIDIA: убираю наши системные файлы" "NVIDIA: removing our system files" "NVIDIA: removendo nossos arquivos de sistema")"
  $SU rm -f "$NV_PROFILE"
  nvidia_old || return 0
  local rebuild=0
  [[ -e ${NV_OLD[1]} || -e ${NV_OLD[2]} ]] && rebuild=1
  $SU rm -f "${NV_OLD[@]}" || return 1
  ((rebuild)) || return 0
  if [[ -x /usr/bin/mkinitcpio ]]; then
    if command -v limine-mkinitcpio >/dev/null; then $SU limine-mkinitcpio; else $SU /usr/bin/mkinitcpio -P; fi
  elif command -v dracut >/dev/null; then
    $SU dracut -f --regenerate-all
  fi
}
