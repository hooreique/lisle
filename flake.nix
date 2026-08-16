{
  description = "Lisle IBus input method";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      home-manager,
      nixpkgs,
      ...
    }:
    let
      system = "x86_64-linux";
      gnomeUnit = "org.freedesktop.IBus.session.GNOME.service";
      pkgs = import nixpkgs { inherit system; };
      lisle = pkgs.callPackage ./nix/package.nix { };
      overlay = final: _prev: {
        lisle = final.callPackage ./nix/package.nix { };
      };
      nixosModule = ./nix/nixos-module.nix;
      homeManagerModule = ./nix/home-manager-module.nix;
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
      homeManagerModuleConfig = home-manager.lib.homeManagerConfiguration {
        inherit pkgs;
        modules = [
          homeManagerModule
          {
            home = {
              username = "lisle-test";
              homeDirectory = "/home/lisle-test";
              stateVersion = "26.05";
            };
            manual.manpages.enable = false;
            programs.lisle.enable = true;
          }
        ];
      };
      homeManagerDconfOnlyConfig = home-manager.lib.homeManagerConfiguration {
        inherit pkgs;
        modules = [
          homeManagerModule
          {
            home = {
              username = "lisle-test";
              homeDirectory = "/home/lisle-test";
              stateVersion = "26.05";
            };
            manual.manpages.enable = false;
            programs.lisle = {
              enable = true;
              ibus.enable = false;
            };
          }
        ];
      };
      nixosIbus = nixosModuleConfig.config.i18n.inputMethod.package;
      homeManagerGeneration = homeManagerModuleConfig.activationPackage;
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
      home-manager-module =
        assert homeManagerModuleConfig.config.home.sessionVariables.GTK_IM_MODULE == "ibus";
        assert
          homeManagerModuleConfig.config.xdg.configFile."systemd/user/${gnomeUnit}".source != null;
        assert
          builtins.any (source: nixpkgs.lib.hasInfix "'lisle'" source)
            homeManagerModuleConfig.config.dconf.settings."org/gnome/desktop/input-sources".sources.value;
        assert !(homeManagerDconfOnlyConfig.config.home.sessionVariables ? GTK_IM_MODULE);
        assert
          !builtins.hasAttr "systemd/user/${gnomeUnit}"
            homeManagerDconfOnlyConfig.config.xdg.configFile;
        assert
          builtins.any (source: nixpkgs.lib.hasInfix "'lisle'" source)
            homeManagerDconfOnlyConfig.config.dconf.settings."org/gnome/desktop/input-sources".sources.value;
        pkgs.runCommand "lisle-home-manager-module" { generation = homeManagerGeneration; } ''
          unit="$generation/home-files/.config/systemd/user/${gnomeUnit}"
          generic_unit="$generation/home-files/.config/systemd/user/org.freedesktop.IBus.session.generic.service"
          wanted_unit="$generation/home-files/.config/systemd/user/gnome-session.target.wants/${gnomeUnit}"
          dbus_service="$generation/home-files/.local/share/dbus-1/services/org.freedesktop.IBus.service"
          unit_source="$(readlink -f "$unit")"
          dbus_source="$(readlink -f "$dbus_service")"
          aggregate="''${unit_source%/share/systemd/user/*}"

          test -L "$unit"
          test -L "$generic_unit"
          test -L "$wanted_unit"
          test -L "$dbus_service"
          test -e "$aggregate/share/ibus/component/lisle.xml"
          grep -F "$aggregate/bin/ibus-daemon" "$unit_source"
          grep -F "Exec=$aggregate/bin/ibus-daemon" "$dbus_source"
          test -e "$generation/home-path/etc/gtk-3.0/immodules.cache"
          test -e "$generation/home-files/.config/autostart/ibus-daemon.desktop"
          touch "$out"
        '';
    in
    {
      inherit homeManagerModule nixosModule overlay;

      overlays = {
        default = overlay;
        lisle = overlay;
      };

      nixosModules = {
        default = nixosModule;
        lisle = nixosModule;
      };

      homeManagerModules = {
        default = homeManagerModule;
        lisle = homeManagerModule;
      };

      packages.${system} = {
        default = lisle;
        inherit ibus-with-lisle lisle;
      };

      checks.${system} = {
        package = lisle;
        inherit
          fmt
          home-manager-module
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
