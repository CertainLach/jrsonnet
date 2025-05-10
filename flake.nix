{
  description = "Jrsonnet";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-24.11";
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
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.shelly.flakeModule ];
      systems = inputs.nixpkgs.lib.systems.flakeExposed;
      perSystem =
        {
          lib,
          self',
          pkgs,
          config,
          system,
          ...
        }:
        let
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rust;
          treefmt = (inputs.treefmt-nix.lib.evalModule pkgs ./treefmt.nix).config.build;
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
            config.allowUnsupportedSystem = true;
          };
          legacyPackages = {
            jsonnetImpls = {
              go-jsonnet = pkgs.callPackage ./nix/go-jsonnet.nix { };
              sjsonnet = pkgs.callPackage ./nix/sjsonnet.nix { };
              jsonnet = pkgs.callPackage ./nix/jsonnet.nix { };
              # I didn't managed to build it, and nixpkgs version is marked as broken
              # haskell-jsonnet = pkgs.callPackage ./nix/haskell-jsonnet.nix { };
              rsjsonnet = pkgs.callPackage ./nix/rsjsonnet.nix { };
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
              inherit (config.legacyPackages.jsonnetImpls)
                go-jsonnet
                sjsonnet
                jsonnet
                rsjsonnet
                ;
              jrsonnetVariants = [
                {
                  drv = jrsonnet.override { forBenchmarks = true; };
                  name = "";
                }
              ];
            };
            benchmarks-quick = pkgs.callPackage ./nix/benchmarks.nix {
              inherit (config.legacyPackages.jsonnetImpls)
                go-jsonnet
                sjsonnet
                jsonnet
                rsjsonnet
                ;
              quick = true;
              jrsonnetVariants = [
                {
                  drv = jrsonnet.override { forBenchmarks = true; };
                  name = "";
                }
              ];
            };
            benchmarks-against-release = pkgs.callPackage ./nix/benchmarks.nix {
              inherit (config.legacyPackages.jsonnetImpls)
                go-jsonnet
                sjsonnet
                jsonnet
                rsjsonnet
                ;
              jrsonnetVariants = [
                {
                  drv = jrsonnet.override { forBenchmarks = true; };
                  name = "current";
                }
                {
                  drv = jrsonnet-nightly.override { forBenchmarks = true; };
                  name = "current-nightly";
                }
                {
                  drv = jrsonnet-release.override { forBenchmarks = true; };
                  name = "release";
                }
              ];
            };
            benchmarks-quick-against-release = pkgs.callPackage ./nix/benchmarks.nix {
              inherit (config.legacyPackages.jsonnetImpls)
                go-jsonnet
                sjsonnet
                jsonnet
                rsjsonnet
                ;
              quick = true;
              jrsonnetVariants = [
                {
                  drv = jrsonnet.override { forBenchmarks = true; };
                  name = "current";
                }
                {
                  drv = jrsonnet-nightly.override { forBenchmarks = true; };
                  name = "current-nightly";
                }
                {
                  drv = jrsonnet-release.override { forBenchmarks = true; };
                  name = "release";
                }
              ];
            };
          };
          shelly.shells.default = {
            factory = craneLib.devShell;
            packages =
              with pkgs;
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
                # kdePackages.kcachegrind
                # kdePackages.massif-visualizer
                # Those packages have too aggressive propagates, bloating env vars and causing
                # rust compiler to fail with collect2: argument list too long error, lol
                (runCommand "kde-tools" { } ''
                  mkdir -p $out/bin
                  ln -s ${kdePackages.kcachegrind}/bin/kcachegrind $out/bin/
                  ln -s ${kdePackages.massif-visualizer}/bin/massif-visualizer $out/bin/
                  ln -s ${hotspot}/bin/hotspot $out/bin/
                '')
              ];
          };
          formatter = treefmt.wrapper;
        };
    };
}
