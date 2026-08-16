{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.lisle;
  defaultPackage = pkgs.callPackage ./package.nix { };

  ibusEngine = lib.types.mkOptionType {
    name = "ibus-engine";
    inherit (lib.types.package) descriptionClass merge;
    check = package:
      lib.types.package.check package
      && lib.attrByPath [ "meta" "isIbusEngine" ] false package;
  };

  ibusWithLisle = pkgs.ibus-with-plugins.override {
    plugins = [ cfg.package ] ++ cfg.ibus.extraEngines;
  };

  gtk3Cache = pkgs.runCommand "lisle-gtk3-immodule-cache" {
    preferLocalBuild = true;
    allowSubstitutes = false;
    buildInputs = [ ibusWithLisle ];
  } ''
    mkdir -p "$out/etc/gtk-3.0"
    GTK_PATH=${ibusWithLisle}/lib/gtk-3.0/ \
      ${pkgs.stdenv.hostPlatform.emulator pkgs.buildPackages} \
      ${lib.getExe' pkgs.gtk3.dev "gtk-query-immodules-3.0"} \
      > "$out/etc/gtk-3.0/immodules.cache"
  '';

  gnomeUnit = "org.freedesktop.IBus.session.GNOME.service";
  genericUnit = "org.freedesktop.IBus.session.generic.service";
in
{
  options.programs.lisle = {
    enable = lib.mkEnableOption "the Lisle IBus input method";

    package = lib.mkOption {
      type = ibusEngine;
      default = defaultPackage;
      defaultText = lib.literalExpression "pkgs.callPackage ./nix/package.nix { }";
      description = "The Lisle package to register as an IBus engine.";
    };

    ibus = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          Whether Home Manager should run a user-scoped IBus containing Lisle.
          Disable this when the NixOS module already registers Lisle system-wide.
        '';
      };

      extraEngines = lib.mkOption {
        type = lib.types.listOf ibusEngine;
        default = [ ];
        example = lib.literalExpression "with pkgs.ibus-engines; [ hangul mozc ]";
        description = ''
          Additional IBus engines to include in the user-scoped IBus package.
          IBus component paths are not additive, so every desired extra engine
          must be listed here.
        '';
      };
    };

    gnome.addToInputSources = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to add Lisle to the GNOME input source list.";
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      assertions = [
        {
          assertion = pkgs.stdenv.hostPlatform.system == "x86_64-linux";
          message = "programs.lisle supports only x86_64-linux";
        }
        {
          assertion = !cfg.ibus.enable || !config.i18n.inputMethod.enable;
          message = ''
            programs.lisle.ibus.enable conflicts with Home Manager's
            i18n.inputMethod; disable one of them
          '';
        }
      ];
    }

    (lib.mkIf cfg.ibus.enable {
      home.packages = [ ibusWithLisle ] ++ lib.optional (
        pkgs.stdenv.hostPlatform.emulatorAvailable pkgs.buildPackages
      ) gtk3Cache;

      home.sessionVariables = {
        GTK_IM_MODULE = "ibus";
        QT_IM_MODULE = "ibus";
        XMODIFIERS = "@im=ibus";
      };

      # NixOS installs its GNOME IBus unit in /etc/systemd/user, which takes
      # precedence over package-provided units in $XDG_DATA_HOME. Put the
      # aggregate's units in $XDG_CONFIG_HOME so the user-owned daemon wins.
      xdg.configFile = {
        "autostart/ibus-daemon.desktop".text = ''
          [Desktop Entry]
          Name=IBus
          Type=Application
          Exec=${ibusWithLisle}/bin/ibus-daemon --daemonize --xim
          NotShowIn=GNOME;KDE;
        '';
        "systemd/user/${gnomeUnit}".source =
          "${ibusWithLisle}/share/systemd/user/${gnomeUnit}";
        "systemd/user/${genericUnit}".source =
          "${ibusWithLisle}/share/systemd/user/${genericUnit}";
        "systemd/user/gnome-session.target.wants/${gnomeUnit}".source =
          "${ibusWithLisle}/share/systemd/user/${gnomeUnit}";
      };

      # User D-Bus activation must resolve to the same aggregate as the units.
      dbus.packages = [ ibusWithLisle ];
    })

    (lib.mkIf cfg.gnome.addToInputSources {
      dconf.settings."org/gnome/desktop/input-sources".sources = lib.mkBefore [
        (lib.hm.gvariant.mkTuple [
          "ibus"
          "lisle"
        ])
      ];
    })
  ]);
}
