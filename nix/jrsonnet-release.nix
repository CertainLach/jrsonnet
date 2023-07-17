{ lib, fetchFromGitHub, rustPlatform, runCommand, makeWrapper }:


rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "before-str-extend";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = pname;
    rev = "777cdf5396004dd5e9447da82c9f081066729d91";
    hash = "sha256-xfNKSjOZM77NB3mJkTY9RC+ClX5KLyk/Q774vWK0goc=";
  };
  cargoHash = "sha256-EJQbOmAD6O5l9YKgd/nFD4Df3PfETQ/ffm2YxxxxW1U=";

  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
