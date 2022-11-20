{ lib, fetchFromGitHub, rustPlatform, runCommand, makeWrapper }:

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
  version = "git";

  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';

  cargoLock = {
    lockFile = ../Cargo.lock;
  };
}
