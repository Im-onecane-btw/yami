# The NVIDIA fixes (component nvidia, the proprietary driver only): a profile
# against niri's VRAM leak, the driver in the initramfs (a picture at the disk password on any
# output) and the GPU encoder in OBS on drivers too old for its NVENC.

do_nvidia_profile() {
  $SU install -Dm644 "$ROOT/system/nvidia/50-niri-vram.json" \
    /etc/nvidia/nvidia-application-profiles-rc.d/50-niri-vram.json
}

# NVIDIA early KMS: without the driver in the initramfs the LUKS prompt and the splash only show
# where the firmware lit a picture (often HDMI only, a black DisplayPort)
NV_MODS="nvidia nvidia_modeset nvidia_uvm nvidia_drm"
do_nvidia_early() {
  modinfo nvidia_drm >/dev/null 2>&1 || return 99
  # a hybrid laptop shows its panel through the other GPU: nothing to gain, and the driver would load early
  ((HYBRID)) && return 99
  $SU install -Dm644 "$ROOT/system/nvidia/yami-nvidia-kms.conf" /etc/modprobe.d/yami-nvidia-kms.conf || return 1
  if [[ -x /usr/bin/mkinitcpio ]]; then
    grep -qs '^[^#]*MODULES.*nvidia_drm' /etc/mkinitcpio.conf /etc/mkinitcpio.conf.d/*.conf && return 0
    printf '# yami: NVIDIA in the initramfs, a picture at the disk password prompt on any output\nMODULES+=(%s)\n' "$NV_MODS" |
      $SU tee /etc/mkinitcpio.conf.d/90-yami-nvidia.conf >/dev/null || return 1
    # CachyOS/Limine wrap mkinitcpio so that boot entries are updated too
    if command -v limine-mkinitcpio >/dev/null; then $SU limine-mkinitcpio; else $SU /usr/bin/mkinitcpio -P; fi
  elif command -v dracut >/dev/null; then
    printf '# yami: NVIDIA in the initramfs\nadd_drivers+=" %s "\ninstall_items+=" /etc/modprobe.d/yami-nvidia-kms.conf "\n' "$NV_MODS" |
      $SU tee /etc/dracut.conf.d/90-yami-nvidia.conf >/dev/null || return 1
    $SU dracut -f --regenerate-all
  else
    return 99
  fi
}

# OBS on Pascal and older: OBS and ffmpeg want NVENC API 13.1 (driver 610+), these cards end on
# the 580 branch (13.0), so OBS lists no NVENC at all. GStreamer's nvcodec works with any driver;
# obs-gstreamer brings it into OBS as «GStreamer Encoder» (nvh264enc, nvh265enc)
do_nvidia_obs() {
  command -v obs-nvenc-test >/dev/null || return 99
  obs-nvenc-test 2>/dev/null | grep -q '^reason=outdated_driver' || return 99
  [[ $PM == pacman ]] && pacman -Si obs-gstreamer >/dev/null 2>&1 || return 99
  pkgs obs-gstreamer gst-plugins-bad
}
