{
  buildGoModule,
  fetchFromGitHub,
  makeWrapper,
}:
let
  pname = "go-jsonnet";
  version = "0.21.0";
  src = fetchFromGitHub {
    owner = "google";
    repo = pname;
    rev = "v${version}";
    hash = "sha256-J92xNDpCidbiSsN6NveS6BX6Tx+qDQqkgm6pjk1wBTQ=";
  };
in
buildGoModule {
  inherit pname version src;

  vendorHash = "sha256-Uh2rAXdye9QmmZuEqx1qeokE9Z9domyHsSFlU7YZsZw=";

  buildInputs = [ makeWrapper ];

  postInstall = ''
    mv $out/bin/jsonnet $out/bin/go-jsonnet
    wrapProgram $out/bin/go-jsonnet --add-flags "--max-stack 200000"
  '';

  passthru = { inherit src; };

  doCheck = false;

  subPackages = [ "cmd/jsonnet" ];
}
