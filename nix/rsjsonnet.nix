{
  fetchFromGitHub,
  rustPlatform,
  makeWrapper,
}:
rustPlatform.buildRustPackage rec {
  pname = "rsjsonnet";
  version = "2026-03-23";

  src = fetchFromGitHub {
    owner = "eduardosm";
    repo = pname;
    rev = "27be31532180c611383ceb2b7f03193ab1253487";
    hash = "sha256-0VM6v1VfQOGUXuYOuh90ta1GaLf1YA+Apm3SkH8CDN4=";
  };

  cargoHash = "sha256-0IDAxm4J2rEqfUGNYoQTP0RPrEZe4YPe2E6TT7A4THo=";

  nativeBuildInputs = [makeWrapper];

  postInstall = ''
    wrapProgram $out/bin/rsjsonnet --add-flags "--max-stack=200000"
  '';
}
