{ lib
, fetchFromGitHub
, rustPlatform
, runCommand
, makeWrapper
, withNightlyFeatures ? false
}:

with lib;

let
  filteredSrc = builtins.path {
    name = "jrsonnet-src-filtered";
    filter = path: type: !(builtins.baseNameOf path == "flake.nix" || builtins.baseNameOf path == "nix");
    path = ../.;
  };

  # for some reason, filteredSrc hash still depends on nix directory contents
  # Moving it into a CA store drops leftover references
  src = runCommand "jrsonnet-src"
    {
      __contentAddressed = true;
    } "cp -r '${filteredSrc}' $out";
in

rustPlatform.buildRustPackage rec {
  inherit src;
  pname = "jrsonnet";
  version = "current${optionalString withNightlyFeatures "-nightly"}";

  cargoTestFlags = [
    "--features=mimalloc,legacy-this-file${optionalString withNightlyFeatures ",nightly"}"
  ];
  cargoBuildFlags = cargoTestFlags;

  nativeBuildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "ass-stroke-0.1.0" = "sha256-muCxjyvgtbK6QDePRQlq29Ey7A22vvCaCMoZH+OUr6U=";
      "boa_gc-0.17.0" = "sha256-4h5nLLdSYgDZNo9g3jx5XUpDBqWhmJ74OgyE4tXdKg8=";
    };
  };
}
