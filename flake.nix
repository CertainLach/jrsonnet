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
          cargoSha256 = "sha256-cez8pJ/uwj+PHAPQwpSB4CKaxcP8Uvv8xguOrVXR2xE=";
        };
      in { 
        defaultPackage = jrsonnet;
        devShell = pkgs.mkShell {};
      });
}
