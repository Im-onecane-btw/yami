# yami: the shell, yami-files, yami-view, yami-play and the login screen yami-greet, built from the
# repository's Cargo.lock. Updates belong to Nix: the `package-managed` feature turns the built-in
# updater off. Fonts, .desktop files, the portal and D-Bus files and the per-user configs go to
# share/, the login screen's session script to libexec/yami (it finds bin/yami-greet from there).
{
  lib,
  rustPlatform,
  pkg-config,
  libxkbcommon,
  linux-pam,
  wayland,
  mpv-unwrapped,
  version,
  src,
}:

rustPlatform.buildRustPackage {
  pname = "yami";
  inherit version src;

  cargoRoot = "shell";
  buildAndTestSubdir = "shell";
  cargoLock.lockFile = ../shell/Cargo.lock;

  buildFeatures = [ "yami-shell/package-managed" ];
  cargoBuildFlags = [ "--workspace" ];
  # the tests draw panels offscreen with the user's fonts and settings: tools/check.sh runs them
  doCheck = false;

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [
    libxkbcommon
    linux-pam
    wayland
  ];

  postInstall = ''
    # `yami <command>`: the same binary, help instead of the shell without a command
    ln -s yami-shell $out/bin/yami

    install -Dm644 -t $out/share/applications config/applications/*.desktop
    install -Dm644 config/portal/yami.portal $out/share/xdg-desktop-portal/portals/yami.portal
    for f in config/portal/*.service; do
      install -Dm644 "$f" "$out/share/dbus-1/services/''${f##*/}"
      substituteInPlace "$out/share/dbus-1/services/''${f##*/}" --replace-fail @BIN@ "$out/bin/yami-files"
    done
    # per-user configs the Home Manager module links or seeds into ~/.config
    install -Dm644 -t $out/share/yami config/niri/config.kdl config/kitty/yami.conf \
      config/portal/niri-portals.conf config/fontconfig/50-yami-pixel.conf
    install -Dm644 -t $out/share/fonts/truetype/yami fonts/*.ttf
    install -Dm644 -t $out/share/licenses/yami fonts/OFL-*.txt LICENSE
    # the login screen: its session script and its two fonts
    install -Dm755 system/greet/greet-session $out/libexec/yami/greet-session
    install -Dm644 -t $out/share/yami-greet/fonts fonts/Tiny5.ttf fonts/DotGothic16-Regular.ttf
  '';

  # yami-play loads libmpv with dlopen; the binary's RUNPATH lets it find the store copy
  postFixup = ''
    patchelf --add-rpath ${lib.makeLibraryPath [ mpv-unwrapped ]} $out/bin/yami-play
  '';

  meta = {
    description = "Lightweight pixel-art Wayland shell for the niri compositor";
    homepage = "https://github.com/Trendorin/yami";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "yami";
  };
}
