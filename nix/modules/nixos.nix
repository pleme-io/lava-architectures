# nix/modules/nixos.nix — auto-generated from lava-architectures.caixa.lisp
# description: "Reusable infrastructure compositions for the lava suite. Hand-authored typed architectures (AwsVpcNetwork, AkeylessAwsIntegration, EksScaleTest, etc.). Pangea-architectures analog — port produces byte-equivalent terraform.json so state files match between pangea+tofu and lava+magma."
{ config, lib, pkgs, ... }:
let
  cfg = config.services.lava-architectures;
in {
  options.services.lava-architectures = {
    enable = lib.mkEnableOption "lava-architectures";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.lava-architectures or null;
    };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
  };
}
