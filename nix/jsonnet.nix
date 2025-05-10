{
  stdenv,
  fetchFromGitHub,
  makeWrapper,
}:
let
  version = "0.21.0";
in
stdenv.mkDerivation {
  inherit version;
  pname = "jsonnet";

  src = fetchFromGitHub {
    owner = "google";
    repo = "jsonnet";
    rev = "refs/tags/v${version}";
    hash = "sha256-QHp0DOu/pqcgN7di219cHzfFb7fWtdGGE6J1ZXgbOGQ=";
  };

  makeFlags = [
    "jsonnet"
  ];

  nativeBuildInputs = [ makeWrapper ];

  installPhase = ''
    mkdir -p $out/bin
    cp jsonnet $out/bin/jsonnet
    wrapProgram $out/bin/jsonnet --add-flags "--max-stack 200000"
  '';
}
