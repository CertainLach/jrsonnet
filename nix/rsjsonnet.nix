{
  fetchFromGitHub,
  rustPlatform,
  makeWrapper,
}:
rustPlatform.buildRustPackage rec {
  pname = "rsjsonnet";
  version = "0.4.0";

  src = fetchFromGitHub {
    owner = "eduardosm";
    repo = pname;
    rev = "v${version}";
    hash = "sha256-Oas/fll5YerHAMI91fTEQqe6praYh4Ro8idsdvzldpA=";
  };

  cargoHash = "sha256-jH2BOvD0Iss34hODhLFHKx5pGMVtkZir7E1bYwjSa8E=";

  nativeBuildInputs = [makeWrapper];

  postInstall = ''
    wrapProgram $out/bin/rsjsonnet --add-flags "--max-stack=200000"
  '';
}
