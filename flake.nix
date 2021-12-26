{
  description = "Dotfiles manager";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    rust-overlay.url = "github:oxalica/rust-overlay";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, pre-commit-hooks, naersk }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs
          {
            inherit system;
            overlays = [ rust-overlay.overlay ];
          };
        rust = ((pkgs.rustChannelOf { date = "2021-11-11"; channel = "nightly"; }).default.override {
          extensions = [ "rust-src" ];
        });
        naersk-lib = naersk.lib."${system}".override {
          rustc = rust;
          cargo = rust;
        };
      in
      rec {
        checks = {
          pre-commit-check = pre-commit-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              nixpkgs-fmt.enable = true;
            };
          };
        };
        defaultPackage = naersk-lib.buildPackage {
          pname = "dotman";
          root = ./.;
          buildInputs = with pkgs; [
            pkgs.sqlite
          ];
        };
        devShell = pkgs.mkShell {
          inherit (checks.pre-commit-check) shellHook;
          nativeBuildInputs = with pkgs;[
            pkgs.binutils
            pkgs.pkgconfig
            pkgs.clang
            pkgs.x11
            pkgs.alsaLib
            pkgs.libudev
            pkgs.sqlite
            rust
            cargo-edit
            go-jsonnet
          ];
        };
      }
    );
}
