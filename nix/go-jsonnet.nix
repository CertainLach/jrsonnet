{
  buildGoModule,
  fetchFromGitHub,
  makeWrapper,
}:
buildGoModule rec {
  pname = "go-jsonnet";
  version = "0.20.0";

  src = fetchFromGitHub {
    owner = "google";
    repo = pname;
    rev = "v${version}";
    hash = "sha256-P69tguBrFF/CSCOfHjCfBT5710oJdhZDh3kMCbc32eE=";
  };
  vendorHash = "sha256-j1fTOUpLx34TgzW94A/BctLrg9XoTtb3cBizhVJoEEI=";

  buildInputs = [makeWrapper];

  postInstall = ''
    mv $out/bin/jsonnet $out/bin/go-jsonnet
    wrapProgram $out/bin/go-jsonnet --add-flags "--max-stack 200000"
  '';

  doCheck = false;

  subPackages = ["cmd/jsonnet"];
}
