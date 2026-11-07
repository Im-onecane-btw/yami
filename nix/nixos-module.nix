# NixOS: programs.yami. The system half of what install.sh does: niri, the services the shell talks
# to (NetworkManager, BlueZ, udisks, gvfs, polkit, power profiles, the keyring), the portals with
# yami's file dialog and screen-share picker, fonts, the commands the shell runs, and the login
# screen: greetd starting yami-greet inside niri. The home half (niri's config, settings) is the
# Home Manager module.
self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.yami;
  run = import ./runtime.nix pkgs;
  niri = config.programs.niri.package;
  # greetd's environment is bare: the session script gets niri, the tools it calls and the folder
  # with the sessions NixOS knows (niri's among them)
  session = pkgs.writeShellScript "yami-greet-session" ''
    export PATH=${
      lib.makeBinPath [
        niri
        pkgs.coreutils
        pkgs.util-linux
        pkgs.systemd
      ]
    }:$PATH
    export XDG_DATA_DIRS=${config.services.displayManager.sessionData.desktops}/share
    exec ${cfg.package}/libexec/yami/greet-session
  '';
in
{
  options.programs.yami = {
    enable = lib.mkEnableOption "yami, the pixel-art desktop on niri";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.yami;
      defaultText = lib.literalExpression "yami.packages.\${system}.yami";
      description = "The yami package (built with updates handed to Nix).";
    };

    loginScreen = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          yami's login screen: greetd starts yami-greet inside niri, with the lock screen's look.
          Turn it off to keep another display manager.
        '';
      };
      user = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "alice";
        description = ''
          Whose shell publishes the look of the login screen (palette, wallpaper, language, screens
          and keyboard layouts) into {file}`/var/lib/yami-greet/look`. Without it the login screen
          keeps its own default look.
        '';
      };
    };

    apps = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "kitty, fastfetch, pavucontrol, btop, video thumbnails and bsdtar, as install.sh puts them.";
    };

    tools = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Night light (wlsunset), text and QR codes from screenshots (tesseract, zbar).";
    };

    appTheme = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "qt6ct and adw-gtk3: Qt and GTK apps in the palette (theme.py writes their configs).";
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        programs.niri.enable = true;

        environment.systemPackages =
          [ cfg.package ]
          ++ run.base
          ++ lib.optionals cfg.apps run.apps
          ++ lib.optionals cfg.tools run.tools
          ++ lib.optionals cfg.appTheme run.look;
        fonts.packages = run.fonts ++ [ cfg.package ];

        # the shell is the polkit agent (the setuid helper in /run/wrappers/bin)
        security.polkit.enable = true;
        services.gvfs.enable = true;
        services.udisks2.enable = true;
        services.gnome.gnome-keyring.enable = lib.mkDefault true;
        services.power-profiles-daemon.enable = lib.mkDefault true;
        networking.networkmanager.enable = lib.mkDefault true;
        hardware.bluetooth.enable = lib.mkDefault true;

        # FileChooser and ScreenCast from yami-files, the rest from GNOME's and GTK's portals; the
        # shell rewrites ~/.config/xdg-desktop-portal/niri-portals.conf (settings → services)
        xdg.portal = {
          enable = true;
          extraPortals = [
            cfg.package
            pkgs.xdg-desktop-portal-gnome
            pkgs.xdg-desktop-portal-gtk
          ];
          config.niri = {
            default = lib.mkDefault [
              "gnome"
              "gtk"
            ];
            "org.freedesktop.impl.portal.Access" = lib.mkDefault [ "gtk" ];
            "org.freedesktop.impl.portal.Notification" = lib.mkDefault [ "gtk" ];
            "org.freedesktop.impl.portal.Secret" = lib.mkDefault [ "gnome-keyring" ];
            "org.freedesktop.impl.portal.FileChooser" = lib.mkDefault [
              "yami"
              "gtk"
            ];
            "org.freedesktop.impl.portal.ScreenCast" = lib.mkDefault [
              "yami"
              "gnome"
            ];
          };
        };
        # «show in folder» (org.freedesktop.FileManager1) starts yami-files
        services.dbus.packages = [ cfg.package ];
      }

      (lib.mkIf cfg.loginScreen.enable {
        services.greetd = {
          enable = true;
          settings.default_session = {
            command = "${session}";
            user = "greeter";
          };
        };
        # what yami-greet reads: the look (the user's shell writes it), its memory and its log
        systemd.tmpfiles.rules =
          [
            "d /var/lib/yami-greet 0755 root root -"
            "d /var/lib/yami-greet/state 0700 greeter greeter -"
            "d /var/lib/yami-greet/log 0755 greeter greeter -"
          ]
          ++ lib.optional (
            cfg.loginScreen.user != null
          ) "d /var/lib/yami-greet/look 0755 ${cfg.loginScreen.user} - -";
      })
    ]
  );
}
