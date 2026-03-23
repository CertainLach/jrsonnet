{
  stdenv,
  fetchFromGitHub,
  makeWrapper,
}:
let
  pname = "cpp-jsonnet";
  version = "2026-03-23";
  src = fetchFromGitHub {
    rev = "d33798d495d50df427dac0dc6934220e366976fb";
    owner = "google";
    repo = "jsonnet";
    hash = "sha256-fpXaYK6WKpXQ0/VbHHsE8ZR/0VpJHmFul/3a6HzBa5o=";
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

  passthru = {
    inherit src;
    jsonnetBench = src;
  };
}
