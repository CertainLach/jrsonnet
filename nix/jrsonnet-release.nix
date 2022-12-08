{ lib, fetchFromGitHub, rustPlatform, runCommand, makeWrapper }:


rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "before-str-extend";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = pname;
    rev = "ccafbf79faf649e0990e277c061be9a2b62ad84c";
    hash = "sha256-LTDIJY9wfv4h5e3/5bONHHBS0qMLKdY6bk6ajKEjG7A=";
  };
  cargoHash = "sha256-LBlJWE3LcbOe/uu19TbLhbUhBKy8DzuDCP4XyuAEmUk=";

  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
