{
  fetchFromGitHub,
  rustPlatform,
  lib,
}:
rustPlatform.buildRustPackage rec {
  pname = "rsjsonnet";
  version = "0.1.1";

  src = fetchFromGitHub {
    owner = "eduardosm";
    repo = pname;
    rev = "v${version}";
    hash = "sha256-C6hZYGllKrKKMwMwss6PK2UD5Zb7bk2v8DrGpWnwP/A=";
  };

  cargoHash = "sha256-TsUN9oUu6S1l9oTaR6nET1ZdRvMrR29bkP3VEDre8aE=";
}
