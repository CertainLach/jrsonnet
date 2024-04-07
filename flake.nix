{
  description = "Jrsonnet";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    crane,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [rust-overlay.overlays.default];
          config.allowUnsupportedSystem = true;
        };
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rust;
      in {
        packages = rec {
          default = jrsonnet;
          go-jsonnet = pkgs.callPackage ./nix/go-jsonnet.nix {};
          sjsonnet = pkgs.callPackage ./nix/sjsonnet.nix {};
          jsonnet = pkgs.callPackage ./nix/jsonnet.nix {};
          # I didn't managed to build it, and nixpkgs version is marked as broken
          # haskell-jsonnet = pkgs.callPackage ./nix/haskell-jsonnet.nix { };

          jrsonnet = pkgs.callPackage ./nix/jrsonnet.nix {
            inherit craneLib;
          };
          jrsonnet-nightly = pkgs.callPackage ./nix/jrsonnet.nix {
            inherit craneLib;
            withNightlyFeatures = true;
          };
          jrsonnet-experimental = pkgs.callPackage ./nix/jrsonnet.nix {
            inherit craneLib;
            withExperimentalFeatures = true;
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
                drv = jrsonnet.override {forBenchmarks = true;};
                name = "";
              }
            ];
          };
          benchmarks-quick = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            quick = true;
            jrsonnetVariants = [
              {
                drv = jrsonnet.override {forBenchmarks = true;};
                name = "";
              }
            ];
          };
          benchmarks-against-release = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            jrsonnetVariants = [
              {
                drv = jrsonnet.override {forBenchmarks = true;};
                name = "current";
              }
              {
                drv = jrsonnet-nightly.override {forBenchmarks = true;};
                name = "current-nightly";
              }
              {
                drv = jrsonnet-release.override {forBenchmarks = true;};
                name = "release";
              }
            ];
          };
          benchmarks-quick-against-release = pkgs.callPackage ./nix/benchmarks.nix {
            inherit go-jsonnet sjsonnet jsonnet;
            quick = true;
            jrsonnetVariants = [
              {
                drv = jrsonnet.override {forBenchmarks = true;};
                name = "current";
              }
              {
                drv = jrsonnet-nightly.override {forBenchmarks = true;};
                name = "current-nightly";
              }
              {
                drv = jrsonnet-release.override {forBenchmarks = true;};
                name = "release";
              }
            ];
          };
        };
        devShells.default = craneLib.devShell {
          nativeBuildInputs = with pkgs; [
            alejandra
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
