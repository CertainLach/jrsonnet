{
  description = "Jrsonnet";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    shelly.url = "github:CertainLach/shelly";
  };
  outputs = inputs @ {
    nixpkgs,
    flake-parts,
    rust-overlay,
    crane,
    shelly,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [shelly.flakeModule];
      systems = ["x86_64-linux" "aarch64-linux" "armv7l-linux" "armv6l-linux" "mingw-w64"];
      perSystem = {
        config,
        system,
        ...
      }: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [rust-overlay.overlays.default];
          config.allowUnsupportedSystem = true;
        };
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rust;
      in {
        legacyPackages = {
          jsonnetImpls = {
            go-jsonnet = pkgs.callPackage ./nix/go-jsonnet.nix {};
            sjsonnet = pkgs.callPackage ./nix/sjsonnet.nix {};
            jsonnet = pkgs.callPackage ./nix/jsonnet.nix {};
            # I didn't managed to build it, and nixpkgs version is marked as broken
            # haskell-jsonnet = pkgs.callPackage ./nix/haskell-jsonnet.nix { };
            rsjsonnet = pkgs.callPackage ./nix/rsjsonnet.nix {};
          };
        };
        packages = rec {
          default = jrsonnet;

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
            inherit (config.legacyPackages.jsonnetImpls) go-jsonnet sjsonnet jsonnet rsjsonnet;
            jrsonnetVariants = [
              {
                drv = jrsonnet.override {forBenchmarks = true;};
                name = "";
              }
            ];
          };
          benchmarks-quick = pkgs.callPackage ./nix/benchmarks.nix {
            inherit (config.legacyPackages.jsonnetImpls) go-jsonnet sjsonnet jsonnet rsjsonnet;
            quick = true;
            jrsonnetVariants = [
              {
                drv = jrsonnet.override {forBenchmarks = true;};
                name = "";
              }
            ];
          };
          benchmarks-against-release = pkgs.callPackage ./nix/benchmarks.nix {
            inherit (config.legacyPackages.jsonnetImpls) go-jsonnet sjsonnet jsonnet rsjsonnet;
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
            inherit (config.legacyPackages.jsonnetImpls) go-jsonnet sjsonnet jsonnet rsjsonnet;
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
        shelly.shells.default = {
          factory = craneLib.devShell;
          packages = with pkgs;
            [
              alejandra
              cargo-edit
              cargo-show-asm
              cargo-outdated
              cargo-watch
              cargo-insta
              lld
              hyperfine
              graphviz
            ]
            ++ lib.optionals (!stdenv.isDarwin) [
              valgrind
              kcachegrind
            ];
        };
      };
    };
}
