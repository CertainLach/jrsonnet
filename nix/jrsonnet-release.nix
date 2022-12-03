{ lib, fetchFromGitHub, rustPlatform, runCommand, makeWrapper }:


rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "d32fe45b8ed28fb39b5359a704922922368af1c0";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = pname;
    rev = version;
    hash = "sha256-R9Xt36bYS5upVDzt8hEifwmfocXpJbIKwvxkoJNEGVc=";
  };
  cargoHash = "sha256-V+KGWeNlUnelofaGzufNPLGDyxazoFrjZ/n391VYYws=";

  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    mv $out/bin/jrsonnet $out/bin/jrsonnet-release
    wrapProgram $out/bin/jrsonnet-release --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
