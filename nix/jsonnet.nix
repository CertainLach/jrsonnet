{ stdenv, lib, jekyll, fetchFromGitHub }:

stdenv.mkDerivation rec {
  pname = "jsonnet";
  version = "0.19.1";

  src = fetchFromGitHub {
    rev = "v${version}";
    owner = "google";
    repo = "jsonnet";
    hash = "sha256-q1MNdbyrx4vvN5woe0o90pPqaNtsZjI5RQ7KJt7rOpU=";
  };

  makeFlags = [
    "jsonnet"
  ];

  installPhase = ''
    mkdir -p $out/bin
    cp jsonnet $out/bin/jsonnet
  '';
}
