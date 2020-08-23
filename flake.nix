{
  description = "Rust jsonnet implementation";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        jrsonnet = pkgs.rustPlatform.buildRustPackage rec {
          pname = "jrsonnet";
          version = "0.1.0";
          src = self;
          cargoSha256 = "120mbfh8wlkyp7dlf86yk0544m9vm0y9awrmny9izsyzzix2ccnx";
        };
      in { defaultPackage = jrsonnet; });
}
