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
  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachSystem (with flake-utils.lib.system; [x86_64-linux x86_64-windows]) (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [rust-overlay.overlays.default];
          config.allowUnsupportedSystem = true;
        };
        lib = pkgs.lib;
        rust =
          (pkgs.rustChannelOf {
            date = "2024-01-10";
            channel = "nightly";
          })
          .default
          .override {
            extensions = ["rust-src" "miri" "rust-analyzer" "clippy"];
          };
      in {
        packages = rec {
          go-jsonnet = pkgs.callPackage ./nix/go-jsonnet.nix {};
          sjsonnet = pkgs.callPackage ./nix/sjsonnet.nix {};
          jsonnet = pkgs.callPackage ./nix/jsonnet.nix {};
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
              {
                drv = jrsonnet.overrideAttrs {forBenchmarks = true;};
                name = "";
              }
            ];
          };
          benchmarks-quick = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            quick = true;
            jrsonnetVariants = [
              {
                drv = jrsonnet.overrideAttrs {forBenchmarks = true;};
                name = "";
              }
            ];
          };
          benchmarks-against-release = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            jrsonnetVariants = [
              {
                drv = jrsonnet.overrideAttrs {forBenchmarks = true;};
                name = "current";
              }
              {
                drv = jrsonnet-nightly.overrideAttrs {forBenchmarks = true;};
                name = "current-nightly";
              }
              {
                drv = jrsonnet-release.overrideAttrs {forBenchmarks = true;};
                name = "release";
              }
            ];
          };
          benchmarks-quick-against-release = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            quick = true;
            jrsonnetVariants = [
              {
                drv = jrsonnet;
                name = "current";
              }
              {
                drv = jrsonnet-nightly;
                name = "current-nightly";
              }
              {
                drv = jrsonnet-release;
                name = "release";
              }
            ];
          };
        };
        packagesCross = lib.genAttrs ["mingwW64"] (crossSystem: let
          callPackage = pkgs.pkgsCross.${crossSystem}.callPackage;
        in {
          jrsonnet = callPackage ./nix/jrsonnet.nix {
            # rustPlatform = pkgs.makeRustPlatform {
            #   rustc = rust;
            #   cargo = rust;
            # };
          };
        });
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            alejandra
            rust
            cargo-edit
            cargo-asm
            cargo-outdated
            cargo-watch
            cargo-insta
            lld
            hyperfine
            graphviz
          ];
        };
      }
    );
}
