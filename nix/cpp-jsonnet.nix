{
  stdenv,
  fetchFromGitHub,
  makeWrapper,
}:
let
  pname = "cpp-jsonnet";
  version = "0.21.0";
  src = fetchFromGitHub {
    rev = "v${version}";
    owner = "google";
    repo = "jsonnet";
    hash = "sha256-QHp0DOu/pqcgN7di219cHzfFb7fWtdGGE6J1ZXgbOGQ=";
  };
in
stdenv.mkDerivation {
  inherit pname version src;

  makeFlags = [
    "jsonnet"
  ];

  nativeBuildInputs = [ makeWrapper ];

  installPhase = ''
    mkdir -p $out/bin
    cp jsonnet $out/bin/jsonnet
    wrapProgram $out/bin/jsonnet --add-flags "--max-stack 200000"
  '';

  passthru = { inherit src; };
}
