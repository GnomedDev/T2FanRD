{
  description = "T2 fan daemon ported to rust";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];
      perSystem =
        {
          pkgs,
          ...
        }:
        {
          formatter = pkgs.nixfmt-rfc-style;
          packages.default = pkgs.rustPlatform.buildRustPackage {
            pname = "t2fanrd";
            version = "0.1.0";

            src = ./.;
            cargoHash = "sha256-RD443SBsDbrJ0Yq3yO23RIZa2Wyi0EnyuX61VmwHZIk=";
          };
        };
    };
}
