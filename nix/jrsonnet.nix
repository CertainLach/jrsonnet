{ lib, fetchFromGitHub, rustPlatform }:

let
  jsonnet = fetchFromGitHub {
    rev = "v${version}";
    owner = "google";
    repo = "jsonnet";
    hash = "sha256-q1MNdbyrx4vvN5woe0o90pPqaNtsZjI5RQ7KJt7rOpU=";
  };
in

rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "git";

  src = ./..;

  cargoTestFlags = [ "--package=jrsonnet" ];
  cargoBuildFlags = [ "--package=jrsonnet" ];

  cargoLock = {
    lockFile = ../Cargo.lock;
  };
}
