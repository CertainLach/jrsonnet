{ lib, buildGo119Module, fetchFromGitHub }:

buildGo119Module rec {
  pname = "go-jsonnet";
  # Latest release is broken: https://github.com/google/go-jsonnet/issues/653
  version = "b4633b66f85e069b105b1ff076d178e4354941bc";

  src = fetchFromGitHub {
    owner = "google";
    repo = "go-jsonnet";
    rev = "${version}";
    hash = "sha256-J+bGdbYo2Ch3ORYD57yJA4jiPiS8IYASZ6kJHhyaqeU=";
  };

  vendorHash = "sha256-j1fTOUpLx34TgzW94A/BctLrg9XoTtb3cBizhVJoEEI=";

  postInstall = ''
    mv $out/bin/jsonnet $out/bin/go-jsonnet
  '';

  doCheck = false;

  subPackages = [ "cmd/jsonnet" ];
}
