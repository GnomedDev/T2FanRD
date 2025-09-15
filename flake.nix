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
      # This makes the package available to your system configuration.
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "t2fanrd";
        version = "0.1.0";
        src = ./.;
        cargoHash = "sha256-FKQYiaOTZxD95AWD2zbVjENzMAPrFl/rzhwbkAgGbx0=";
      };

      # This makes the NixOS module available for import.
      nixosModules.t2fanrd = { config, lib, ... }:
        let
          cfg = config.services.t2fanrd;

          # A helper function to generate the fan configuration string.
          fanConfig = fan: ''
            [${fan.name}]
            low_temp=${toString fan.lowTemp}
            high_temp=${toString fan.highTemp}
            speed_curve=${fan.speedCurve}
            always_full_speed=${if fan.alwaysFullSpeed then "true" else "false"}
          '';
        in
          {
            options.services.t2fanrd = {
              enable = lib.mkEnableOption "t2fanrd daemon to manage fan curves for T2 Macs";

              fans = lib.mkOption {
                type = lib.types.listOf
                  (lib.types.submodule ({ lib, ... }: {
                    options = {
                      name = lib.mkOption {
                        type = lib.types.str;
                        description = "The name of the fan section in the configuration file (e.g., 'Fan1').";
                      };

                      lowTemp = lib.mkOption {
                        type  = lib.types.int;
                        default = 55;
                        example = 40;
                        description = "Temperature in Celsius that will trigger a higher fan speed.";
                      };

                      highTemp = lib.mkOption {
                        type  = lib.types.int;
                        default = 75;
                        example = 80;
                        description = "Temperature in Celsius that will trigger a higher fan speed.";
                      };

                      speedCurve = lib.mkOption {
                        type = lib.types.enum [ "linear" "exponential" "logarithmic" ];
                        default = "linear";
                        example = "logarithmic";
                        description = "Sets the fan speed curve.";
                      };

                      alwaysFullSpeed = lib.mkOption {
                        type = lib.types.bool;
                        default = false;
                        example = true;
                        description = "If true, the fan will be at max speed regardless of temperature.";
                      };
                    };
                  }));
              };
            };

            config = lib.mkIf cfg.enable {
              systemd.services.t2fanrd = {
                description = "T2FanRD daemon to manage the fans on a T2 Mac";
                wantedBy = [ "multi-user.target" ];
                serviceConfig = {
                  Type = "exec";
                  ExecStart = "${self.packages.${system}.default}/bin/t2fanrd";
                  Restart = "always";
                };
                # https://nixos.org/manual/nixos/stable/options#opt-systemd.services._name_.reloadTriggers
                restartTriggers = [ config.environment.etc."t2fand.conf".source ];
              };
              environment.etc."t2fand.conf".text = builtins.concatStringsSep "\n" (map fanConfig cfg.fans);
            };
          };
    };
}
