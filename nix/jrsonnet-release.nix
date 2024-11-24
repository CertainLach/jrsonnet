{
  fetchFromGitHub,
  rustPlatform,
  makeWrapper,
  # This derivation should only be used for benchmarks-against-release task
  forBenchmarks ? true,
  _unused ? forBenchmarks,
}:
rustPlatform.buildRustPackage rec {
  pname = "jrsonnet";
  version = "release";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = pname;
    rev = "a31a8ef0c0189f88ed32d84cb2dcae3b7d7861af";
    hash = "sha256-SJIJG+a+g1QcZIsaXwWXONufKETQZLBSBxE13Mbprus=";
  };
  cargoHash = "sha256-lLT3ihnq5akUZt2iWLFs4pJDNrd18fXxZMZ91MXF0jU=";

  cargoTestFlags = ["--package=jrsonnet --features=mimalloc"];
  cargoBuildFlags = ["--package=jrsonnet --features=mimalloc"];

  buildInputs = [makeWrapper];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000"
  '';
}
