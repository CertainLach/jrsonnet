{
  buildGoModule,
  fetchFromGitHub,
  makeWrapper,
}:
let
  pname = "go-jsonnet";
  version = "2026-03-23";
  src = fetchFromGitHub {
    owner = "google";
    repo = pname;
    rev = "b5ef4cd9c4e24f2f14a68ef3bda0ca3079e11e78";
    hash = "sha256-htC8671r74E26J42eubcFL4lPOURIdSK0P7GjZOWhao=";
  };
in
buildGoModule {
  inherit pname version src;

  vendorHash = "sha256-uFCvMmiZVaRYhaORI92W0pkDjDZNiWIcop70FssJiZo=";

  buildInputs = [ makeWrapper ];

  postInstall = ''
    mv $out/bin/jsonnet $out/bin/go-jsonnet
    wrapProgram $out/bin/go-jsonnet --add-flags "--max-stack 200000"
  '';

  passthru = {
    inherit src;
    goJsonnetBench = src + "/builtin-benchmarks";
  };

  doCheck = false;

  subPackages = [ "cmd/jsonnet" ];
}
