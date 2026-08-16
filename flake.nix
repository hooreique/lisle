{
  description = "Lisle IBus input method";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      nixpkgs,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      lisle = pkgs.callPackage ./nix/package.nix { };
      overlay = final: _prev: {
        lisle = final.callPackage ./nix/package.nix { };
      };
      nixosModule = ./nix/nixos-module.nix;
      ibus-with-lisle = pkgs.ibus-with-plugins.override {
        plugins = [ lisle ];
      };
      fmt = pkgs.runCommand "lisle-fmt" {
        src = lisle.src;
        nativeBuildInputs = [
          pkgs.cargo
          pkgs.rustfmt
        ];
      } ''
        cp -r "$src" source
        chmod -R u+w source
        cd source
        cargo fmt --all -- --check
        touch "$out"
      '';
      ibus-smoke = pkgs.runCommand "lisle-ibus-smoke" {
        DBUS_SESSION_CONF = "${pkgs.dbus}/share/dbus-1/session.conf";
        nativeBuildInputs = [
          ibus-with-lisle
          pkgs.bash
          pkgs.coreutils
          pkgs.dbus
          pkgs.glib
          pkgs.gnugrep
        ];
      } ''
        ${pkgs.bash}/bin/bash ${./tests/ibus-daemon-smoke.sh}
        touch "$out"
      '';
      nixosModuleConfig = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          nixosModule
          {
            programs.lisle.enable = true;
            system.stateVersion = "26.05";
          }
        ];
      };
      nixosIbus = nixosModuleConfig.config.i18n.inputMethod.package;
      nixos-module =
        assert nixosModuleConfig.config.i18n.inputMethod.enable;
        assert nixosModuleConfig.config.i18n.inputMethod.type == "ibus";
        assert
          builtins.any (engine: toString engine == toString lisle)
            nixosModuleConfig.config.i18n.inputMethod.ibus.engines;
        pkgs.runCommand "lisle-nixos-module" { ibus = nixosIbus; } ''
          test -e "$ibus/share/ibus/component/lisle.xml"
          touch "$out"
        '';
    in
    {
      inherit nixosModule overlay;

      overlays = {
        default = overlay;
        lisle = overlay;
      };

      nixosModules = {
        default = nixosModule;
        lisle = nixosModule;
      };

      packages.${system} = {
        default = lisle;
        inherit ibus-with-lisle lisle;
      };

      checks.${system} = {
        package = lisle;
        inherit
          fmt
          ibus-smoke
          nixos-module
          ;
      };

      devShells.${system}.default = pkgs.mkShell {
        # Keep build and check dependencies aligned with the package. This supplies
        # cargo, rustc, Clippy, libxml2, and libxkbcommon.
        inputsFrom = [ lisle ];
        packages = [
          pkgs.dbus
          pkgs.ibus
          pkgs.rustfmt
        ];
        RUST_BACKTRACE = "1";
      };
    };
}
