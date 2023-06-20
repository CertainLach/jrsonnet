{
  description = "Jrsonnet";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };
  outputs = { nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rust = ((pkgs.rustChannelOf { date = "2023-05-07"; channel = "nightly"; }).default.override {
          extensions = [ "rust-src" "miri" "rust-analyzer" ];
        });
      in
      {
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
          jrsonnet-nightly = pkgs.callPackage ./nix/jrsonnet.nix {
            rustPlatform = pkgs.makeRustPlatform {
              rustc = rust;
              cargo = rust;
            };
            withNightlyFeatures = true;
          };
          jrsonnet-release = pkgs.callPackage ./nix/jrsonnet-release.nix {
            rustPlatform = pkgs.makeRustPlatform {
              rustc = rust;
              cargo = rust;
            };
          };

          benchmarks = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            jrsonnetVariants = [
              { drv = jrsonnet; name = ""; }
            ];
          };
          benchmarks-quick = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            quick = true;
            jrsonnetVariants = [
              { drv = jrsonnet; name = ""; }
            ];
          };
          benchmarks-against-release = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            jrsonnetVariants = [
              { drv = jrsonnet; name = "current"; }
              { drv = jrsonnet-nightly; name = "current-nightly"; }
              { drv = jrsonnet-release; name = "before-str-extend"; }
            ];
          };
          benchmarks-quick-against-release = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            quick = true;
            jrsonnetVariants = [
              { drv = jrsonnet; name = "current"; }
              { drv = jrsonnet-nightly; name = "current-nightly"; }
              { drv = jrsonnet-release; name = "before-str-extend"; }
            ];
          };
        };
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            just
            rust
            cargo-edit
            cargo-asm
            cargo-outdated
            lld
            hyperfine
            graphviz
          ] ++ lib.optionals (!stdenv.isDarwin) [
            valgrind
            kcachegrind
          ];
        };
      }
    );
}
