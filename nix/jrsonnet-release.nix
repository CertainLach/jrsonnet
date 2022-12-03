{ lib, fetchFromGitHub, rustPlatform, runCommand, makeWrapper }:


rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "before-str-extend";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = pname;
    rev = "d32fe45b8ed28fb39b5359a704922922368af1c0";
    hash = "sha256-R9Xt36bYS5upVDzt8hEifwmfocXpJbIKwvxkoJNEGVc=";
  };
  cargoHash = "sha256-j2sUIzvK66jn8ajmMsXXHstw79jCLog93XCQj1qjAN8=";

  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
