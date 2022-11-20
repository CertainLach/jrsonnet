{
  description = "Jrsonnet";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs = { nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rust = ((pkgs.rustChannelOf { date = "2022-11-10"; channel = "nightly"; }).default.override {
          extensions = [ "rust-src" "miri" ];
        });
      in
      rec {
        packages = rec {
          go-jsonnet = pkgs.callPackage ./nix/go-jsonnet.nix { };
          sjsonnet = pkgs.callPackage ./nix/sjsonnet.nix { };
          jsonnet = pkgs.callPackage ./nix/jsonnet.nix { };
          # I didn't managed to build it, and nixpkgs version is marked as broken
          # haskell-jsonnet = pkgs.callPackage ./nix/haskell-jsonnet.nix { };
          jrsonnet = pkgs.callPackage ./nix/jrsonnet.nix {
            rustPlatform = pkgs.makeRustPlatform {
              rustc = rust;
              cargo = rust;
            };
          };

          benchmarks = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet jrsonnet;
          };
        };
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs;[
            rust
            cargo-edit
            lld
            hyperfine
            valgrind
          ];
        };
      }
    );
}
