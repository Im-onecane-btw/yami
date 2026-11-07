{
  description = "yami: a lightweight pixel-art Wayland desktop for the niri compositor";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAll = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      version = lib.fileContents ./VERSION;
      # only what the build reads: a change to docs, CI or the installer does not rebuild yami
      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./LICENSE
          ./VERSION
          ./config
          ./fonts
          ./system/greet
          ./tools/theme.py
          (lib.fileset.difference ./shell (lib.fileset.maybeMissing ./shell/target))
        ];
      };
      packagesFor = pkgs: {
        yami = pkgs.callPackage ./nix/package.nix { inherit version src; };
      };
    in
    {
      packages = forAll (
        pkgs:
        packagesFor pkgs
        // {
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.yami;
        }
      );

      # `nix flake check`: the package builds, the NixOS module evaluates (greetd runs yami-greet, the
      # portals, services and tools are set)
      checks = forAll (
        pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
          c =
            (nixpkgs.lib.nixosSystem {
              modules = [
                self.nixosModules.default
                {
                  nixpkgs.hostPlatform = system;
                  programs.yami = {
                    enable = true;
                    loginScreen.user = "yami";
                  };
                  users.users.yami.isNormalUser = true;
                  fileSystems."/" = {
                    device = "/dev/null";
                    fsType = "ext4";
                  };
                  boot.loader.grub.enable = false;
                  system.stateVersion = "25.11";
                }
              ];
            }).config;
        in
        {
          inherit (self.packages.${system}) yami;
          nixos-module = pkgs.writeText "yami-nixos-module.json" (
            builtins.toJSON {
              greeter = c.services.greetd.settings.default_session.command;
              tmpfiles = c.systemd.tmpfiles.rules;
              portals = map (p: p.name) c.xdg.portal.extraPortals;
              packages = map (p: p.name) c.environment.systemPackages;
            }
          );
        }
      );

      nixosModules.default = import ./nix/nixos-module.nix self;
      homeManagerModules.default = import ./nix/hm-module.nix self;

      overlays.default = _final: prev: packagesFor prev;

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.yami ];
          packages = with pkgs; [
            cargo
            rustc
            clippy
            python3
            shellcheck
          ];
          # yami-play finds libmpv at run time
          LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.mpv-unwrapped ];
        };
      });

      formatter = forAll (pkgs: pkgs.nixfmt);
    };
}
