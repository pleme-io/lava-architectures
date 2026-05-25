# nix/modules/darwin.nix — auto-generated from lava-architectures.caixa.lisp
{ config, lib, pkgs, ... }:
let cfg = config.services.lava-architectures; in {
  options.services.lava-architectures = {
    enable = lib.mkEnableOption "lava-architectures";
    package = lib.mkOption { type = lib.types.package; default = pkgs.lava-architectures or null; };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
  };
}
