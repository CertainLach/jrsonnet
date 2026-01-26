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

    crane = {
      url = "github:ipetkov/crane";
    };

    shelly = {
      url = "github:CertainLach/shelly";
    };

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    crane,
    flake-parts,
    nixpkgs,
    rust-overlay,
    shelly,
    treefmt-nix,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        shelly.flakeModule
        treefmt-nix.flakeModule
      ];

      systems = nixpkgs.lib.systems.flakeExposed;

      perSystem = {
        config,
        lib,
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

          benchmarks = pkgs.callPackage ./nix/benchmarks.nix {
            inherit (config.legacyPackages.jsonnetImpls) go-jsonnet sjsonnet jsonnet rsjsonnet;
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

          jrsonnet = pkgs.callPackage ./nix/jrsonnet.nix {
            inherit craneLib;
          };

          jrsonnet-experimental = pkgs.callPackage ./nix/jrsonnet.nix {
            inherit craneLib;
            withExperimentalFeatures = true;
          };

          jrsonnet-nightly = pkgs.callPackage ./nix/jrsonnet.nix {
            inherit craneLib;
            withNightlyFeatures = true;
          };

          jrsonnet-release = pkgs.callPackage ./nix/jrsonnet-release.nix {
            rustPlatform = pkgs.makeRustPlatform {
              rustc = rust;
              cargo = rust;
            };
          };

          rtk = pkgs.callPackage ./nix/rtk.nix {
            inherit craneLib;
          };
          rtk-benchmarks = pkgs.callPackage ./nix/rtk-benchmarks.nix {
            inherit (config) packages;
          };

          tanka = pkgs.callPackage ./nix/tanka.nix {};
        };

        apps = {
          rtk-benchmarks = {
            type = "app";
            program = "${config.packages.rtk-benchmarks}/bin/rtk-benchmarks";
          };
        };

        treefmt = {
          projectRootFile = "flake.nix";
          programs = {
            alejandra.enable = true;
            rustfmt = {
              enable = true;
              package = pkgs.rust-bin.nightly.latest.rustfmt;
              edition = "2021";
            };
            shfmt.enable = true;
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
            ++ config.packages.rtk.testInputs
            ++ lib.optionals (!stdenv.isDarwin) [
              valgrind
              kdePackages.kcachegrind
            ];
        };
      };
    };
}
