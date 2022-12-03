{ lib, fetchFromGitHub, rustPlatform, runCommand, makeWrapper }:


rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "5f0f8de9f52f961e2ff162e0a3fd4ca20a275f1d";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = pname;
    rev = version;
    hash = lib.fakeHash;
  };

  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    mv $out/bin/jrsonnet $out/bin/jrsonnet-release
    wrapProgram $out/bin/jrsonnet-release --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
