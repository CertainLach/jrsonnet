{ stdenv, lib, jekyll, fetchFromGitHub, makeWrapper }:

stdenv.mkDerivation rec {
  pname = "jsonnet";
  version = "0.20.0";

  src = fetchFromGitHub {
    rev = "v${version}";
    owner = "google";
    repo = pname;
    hash = "sha256-FtVJE9alEl56Uik+nCpJMV5DMVVmRCnE1xMAiWdK39Y=";
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
