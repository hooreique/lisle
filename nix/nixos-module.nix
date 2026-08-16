{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.lisle;
  defaultPackage = pkgs.callPackage ./package.nix { };
in
{
  options.programs.lisle = {
    enable = lib.mkEnableOption "the Lisle IBus input method";

    package = lib.mkOption {
      type = lib.types.package;
      default = defaultPackage;
      defaultText = lib.literalExpression "pkgs.callPackage ./nix/package.nix { }";
      description = "The Lisle package to register as an IBus engine.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = pkgs.stdenv.hostPlatform.system == "x86_64-linux";
        message = "programs.lisle supports only x86_64-linux";
      }
      {
        assertion = config.i18n.inputMethod.enable;
        message = "programs.lisle requires i18n.inputMethod.enable = true";
      }
      {
        assertion = config.i18n.inputMethod.type == "ibus";
        message = ''programs.lisle requires i18n.inputMethod.type = "ibus"'';
      }
    ];

    i18n.inputMethod = {
      enable = lib.mkDefault true;
      type = lib.mkDefault "ibus";
      ibus.engines = [ cfg.package ];
    };
  };
}
