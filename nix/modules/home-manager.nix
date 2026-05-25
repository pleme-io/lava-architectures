# nix/modules/home-manager.nix — auto-generated from lava-architectures.caixa.lisp
{ config, lib, pkgs, ... }:
let cfg = config.programs.lava-architectures; in {
  options.programs.lava-architectures = {
    enable = lib.mkEnableOption "lava-architectures";
    package = lib.mkOption { type = lib.types.package; default = pkgs.lava-architectures or null; };
  };
  config = lib.mkIf cfg.enable { home.packages = [ cfg.package ]; };
}
