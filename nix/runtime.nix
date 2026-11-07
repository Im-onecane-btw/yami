# The programs the shell runs (the installer's package lists, installer/packages.sh, in nixpkgs names).
# System services (NetworkManager, BlueZ, udisks, gvfs, polkit, power profiles) belong to the NixOS
# module; these are commands in PATH. Shared by the NixOS and the Home Manager module.
pkgs: {
  base = with pkgs; [
    (python3.withPackages (ps: [ ps.pillow ])) # theme.py: the palette for other apps, wallpaper, cursor
    wl-clipboard
    cliphist
    grim
    brightnessctl
    playerctl
    pulseaudio # pactl and parec on PipeWire
    glib # gio: drives, phones and network places
    curl
    xwayland-satellite
  ];
  apps = with pkgs; [
    kitty
    fastfetch
    pavucontrol
    btop
    ffmpegthumbnailer
    libarchive # bsdtar
  ];
  tools = with pkgs; [
    wlsunset
    (tesseract.override { enableLanguages = [ "eng" "rus" ]; })
    zbar
  ];
  look = with pkgs; [
    qt6Packages.qt6ct
    adw-gtk3
    papirus-icon-theme
  ];
  fonts = with pkgs; [
    noto-fonts
    noto-fonts-cjk-sans
    noto-fonts-color-emoji
  ];
}
