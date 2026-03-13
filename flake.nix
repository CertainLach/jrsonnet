{
  description = "Jrsonnet";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-25.11";
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
  outputs =
    inputs@{
      nixpkgs,
      flake-parts,
      rust-overlay,
      crane,
      shelly,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ shelly.flakeModule ];
      systems = inputs.nixpkgs.lib.systems.flakeExposed;
      perSystem =
        {
          config,
          system,
          ...
        }:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
            config.allowUnsupportedSystem = true;
          };
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain rust;
        in
        {
          legacyPackages = {
            jsonnetImpls = {
              go-jsonnet = pkgs.callPackage ./nix/go-jsonnet.nix { };
              sjsonnet = pkgs.callPackage ./nix/sjsonnet.nix { };
              cpp-jsonnet = pkgs.callPackage ./nix/cpp-jsonnet.nix { };
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
                cpp-jsonnet
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
                cpp-jsonnet
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
                cpp-jsonnet
                rsjsonnet
                ;
              jrsonnetVariants = [
                {
                  drv = jrsonnet.override { forBenchmarks = true; };
                  name = "current";
                }
                {
                  drv = jrsonnet-experimental.override { forBenchmarks = true; };
                  name = "current-experimental";
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
                cpp-jsonnet
                rsjsonnet
                ;
              quick = true;
              jrsonnetVariants = [
                {
                  drv = jrsonnet.override { forBenchmarks = true; };
                  name = "current";
                }
                {
                  drv = jrsonnet-experimental.override { forBenchmarks = true; };
                  name = "current-experimental";
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
                cargo-edit
                cargo-outdated
                cargo-watch
                cargo-insta
                cargo-hack
                lld
                hyperfine
                graphviz
              ]
              ++ lib.optionals (!stdenv.isDarwin) [
                valgrind
              ];
          };
        };
    };
}
