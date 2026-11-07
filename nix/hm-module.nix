# Home Manager: programs.yami. The home half of what install.sh does. The shell edits its own files
# at run time (settings, keys, screens, portals), so those are seeded once and stay writable; only
# files the shell never writes are links. Based on the module TheCodedKid wrote for #1.
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
  cfgDir = config.xdg.configHome;
  share = "${cfg.package}/share/yami";
  json = pkgs.formats.json { };
  mimeOf =
    app:
    let
      line = lib.findFirst (lib.hasPrefix "MimeType=") "MimeType=" (
        lib.splitString "\n" (builtins.readFile "${self}/config/applications/${app}.desktop")
      );
    in
    lib.filter (m: m != "") (lib.splitString ";" (lib.removePrefix "MimeType=" line));
  defaults = lib.genAttrs [ "yami-files" "yami-view" "yami-play" ] (app: "${app}.desktop");
in
{
  options.programs.yami = {
    enable = lib.mkEnableOption "yami, the pixel-art shell for niri";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.yami;
      defaultText = lib.literalExpression "yami.packages.\${system}.yami";
      description = "The yami package (built with updates handed to Nix).";
    };

    settings = lib.mkOption {
      inherit (json) type;
      default = { };
      example = lib.literalExpression ''
        {
          palette = "sakura";
          lang = "en";
          light = false;
        }
      '';
      description = ''
        Keys of {file}`~/.config/yami-shell.json` (camelCase, see docs/CONFIG.md). Merged into the
        file on every activation: these keys win, everything else set in the settings panel stays.
      '';
    };

    niriConfig = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Install yami's {file}`~/.config/niri/config.kdl` (it starts the shell and includes the
        files the shell writes). Like install.sh: it is copied, not linked, and replaced on
        activation only while {file}`~/.config/niri/.yami` marks it as yami's; your own changes
        go into {file}`user.kdl`. An existing config.kdl without the marker is left alone. Turn
        this off when niri's config comes from elsewhere and add the
        `include optional=true "yami-….kdl"` lines from yami's config.kdl yourself.
      '';
    };

    kitty = lib.mkOption {
      type = lib.types.bool;
      default = config.programs.kitty.enable;
      defaultText = lib.literalExpression "config.programs.kitty.enable";
      description = "Link yami's kitty look and include it from programs.kitty.";
    };

    defaultApplications = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        yami-files for folders, yami-view for pictures and yami-play for video and music in
        xdg.mimeApps (takes effect with `xdg.mimeApps.enable`).
      '';
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = run.base ++ run.apps ++ run.tools;
      defaultText = lib.literalMD "the commands the shell calls (`yami doctor` lists them)";
      description = ''
        Programs the shell runs. On NixOS the system module installs them too; system services
        (NetworkManager, gvfs, udisks, polkit, greetd) belong there.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ] ++ cfg.extraPackages;

    fonts.fontconfig.enable = true;
    xdg.configFile."fontconfig/conf.d/50-yami-pixel.conf".source = "${share}/50-yami-pixel.conf";

    # the file dialog and screen-share portal and «show in folder», where install.sh puts them
    xdg.dataFile = {
      "xdg-desktop-portal/portals/yami.portal".source =
        "${cfg.package}/share/xdg-desktop-portal/portals/yami.portal";
      "dbus-1/services/org.freedesktop.impl.portal.desktop.yami.service".source =
        "${cfg.package}/share/dbus-1/services/org.freedesktop.impl.portal.desktop.yami.service";
      "dbus-1/services/org.freedesktop.FileManager1.service".source =
        "${cfg.package}/share/dbus-1/services/org.freedesktop.FileManager1.service";
    };

    xdg.configFile."kitty/yami.conf" = lib.mkIf cfg.kitty { source = "${share}/yami.conf"; };
    programs.kitty.extraConfig = lib.mkIf cfg.kitty (lib.mkAfter "include yami.conf");

    xdg.mimeApps.defaultApplications = lib.mkIf cfg.defaultApplications (
      {
        "inode/directory" = defaults.yami-files;
      }
      // lib.genAttrs (mimeOf "yami-view") (_: defaults.yami-view)
      // lib.genAttrs (mimeOf "yami-play") (_: defaults.yami-play)
    );

    home.activation.yami = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      yami_cfg=${lib.escapeShellArg cfgDir}

      # settings: declared keys over what the panel saved
      ${lib.optionalString (cfg.settings != { }) ''
        yami_json="$yami_cfg/yami-shell.json"
        run mkdir -p "$yami_cfg"
        if [[ ! -e "$yami_json" && ! -v DRY_RUN ]]; then
          echo '{}' >"$yami_json"
        fi
        if [[ -e "$yami_json" ]] && ! ${pkgs.jq}/bin/jq -e 'type == "object"' "$yami_json" >/dev/null 2>&1; then
          warnEcho "yami: $yami_json is not a JSON object, programs.yami.settings not applied"
        elif [[ ! -v DRY_RUN ]]; then
          ${pkgs.jq}/bin/jq --indent 2 -s '.[0] * .[1]' "$yami_json" ${json.generate "yami-shell.json" cfg.settings} >"$yami_json.tmp" \
            && mv -f "$yami_json.tmp" "$yami_json"
        fi
      ''}

      # portals: the shell rewrites this file (settings → services), so a copy, once
      if [[ ! -e "$yami_cfg/xdg-desktop-portal/niri-portals.conf" ]]; then
        run ${pkgs.coreutils}/bin/install -Dm644 ${share}/niri-portals.conf "$yami_cfg/xdg-desktop-portal/niri-portals.conf"
      fi

      ${lib.optionalString cfg.niriConfig ''
        yami_niri="$yami_cfg/niri"
        if [[ ! -e "$yami_niri/config.kdl" || ( -f "$yami_niri/.yami" && ! -L "$yami_niri/config.kdl" ) ]]; then
          run ${pkgs.coreutils}/bin/install -Dm644 ${share}/config.kdl "$yami_niri/config.kdl"
          run touch "$yami_niri/.yami"
          if [[ ! -e "$yami_niri/user.kdl" && ! -v DRY_RUN ]]; then
            printf '%s\n' "// your tweaks on top of yami: binds, window rules, outputs. Included after the shell's" \
              "// files, so what you write here wins. Reference: https://github.com/YaLTeR/niri/wiki" >"$yami_niri/user.kdl"
          fi
        else
          warnEcho "yami: $yami_niri/config.kdl is not yami's (no .yami marker), left as it is"
        fi
        # keys and window rules before the first login; the shell keeps them current
        [[ -e "$yami_niri/yami-binds.kdl" ]] || run ${cfg.package}/bin/yami-shell binds write
        if [[ ! -e "$yami_niri/yami-fx.kdl" && ! -v DRY_RUN ]]; then
          ${cfg.package}/bin/yami-shell config kdl >"$yami_niri/yami-fx.kdl"
        fi
      ''}
    '';
  };
}
