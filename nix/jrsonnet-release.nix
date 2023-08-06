{ lib, fetchFromGitHub, rustPlatform, runCommand, makeWrapper }:


rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "pre9";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = pname;
    rev = "5dc3b98bcc3b9848031f17165bcc2e86e8a65ba3";
    hash = "sha256-KM1yqsFzt7Vj4xiEzJJiuFaG49/utF80r9A2dSwCAjo=";
  };
  cargoHash = "sha256-y2YiktT1h263vpFaC+kRL8yaAWQThhEkS+NSQ6B6Ylk=";


  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
