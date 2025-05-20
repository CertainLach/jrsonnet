{
  fetchFromGitHub,
  rustPlatform,
  makeWrapper,
}: let
  pname = "rsjsonnet";
  version = "0.4.0-git";
in
  rustPlatform.buildRustPackage {
    inherit pname version;

    src = fetchFromGitHub {
      owner = "eduardosm";
      repo = pname;
      rev = "deac8457b9d1648b0f0b559b6cf8ce34854927a9";
      hash = "sha256-fGZZsujTAtKiw9IRFXZpq8QPWr98Q54cCHzXoo33ZAk=";
    };

    cargoHash = "sha256-m1Jjir1hxEZsowP6qMIzDtJUBnDstFghwzoRHJA8msM=";

    nativeBuildInputs = [makeWrapper];

    postInstall = ''
      wrapProgram $out/bin/rsjsonnet --add-flags "--max-stack=200000"
    '';
  }
