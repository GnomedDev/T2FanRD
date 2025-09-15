{
  description = "NixOS module and package for t2fanrd";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux"; # As this is for T2 Macs, only x86_64 needed.
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "t2fanrd";
        version = "0.1.0";
        src = ./.;
        cargoHash = "sha256-FKQYiaOTZxD95AWD2zbVjENzMAPrFl/rzhwbkAgGbx0=";
      };
      nixosModules.t2fanrd = { config, lib, ... }: {
        options.services.t2fanrd = {
          enable = lib.mkEnableOption "t2fanrd daemon to manage fan curves for T2 Macs";

          config = lib.mkOption {
            type = lib.types.attrsOf (lib.types.submodule {
              options = {

                low_temp = lib.mkOption {
                  type  = lib.types.int;
                  default = 55;
                  example = 40;
                  description = "Temperature in Celsius that will trigger a higher fan speed.";
                };

                high_temp = lib.mkOption {
                  type  = lib.types.int;
                  default = 75;
                  example = 80;
                  description = "Temperature in Celsius that will trigger a higher fan speed.";
                };

                speed_curve = lib.mkOption {
                  type = lib.types.enum [ "linear" "exponential" "logarithmic" ];
                  default = "linear";
                  example = "logarithmic";
                  description = "Sets the fan speed curve.";
                };

                always_full_speed = lib.mkOption {
                  type = lib.types.bool;
                  default = false;
                  example = true;
                  description = "If true, the fan will be at max speed regardless of temperature.";
                };
              };
            });
            default = {};
            description = ''
             An attribute set where each attribute is a fan to configure
             with its settings.
            '';
            example = lib.literalExample ''{
              Fan1 = {
                low_temp = 40;
                high_temp = 70;
                speed_curve = "linear";
                always_full_speed = false;
              };
              Fan2 = {
                low_temp = 40;
                high_temp = 70;
                speed_curve = "linear";
                always_full_speed = false;
              };
            };'';
          };
        };

        config = lib.mkIf config.services.t2fanrd.enable {
          systemd.services.t2fanrd = {
            description = "T2FanRD daemon to manage the fans on a T2 Mac";
            wantedBy = [ "multi-user.target" ];
            serviceConfig = {
              Type = "exec";
              ExecStart = "${self.packages.${system}.default}/bin/t2fanrd";
              Restart = "always";
            };
          };
          environment.etc."t2fand.conf".source = ((pkgs.formats.toml { }).generate "t2fand.conf" config.services.t2fanrd.config);
        };
      };
    };
}
