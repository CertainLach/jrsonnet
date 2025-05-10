{
  fetchFromGitHub,
  rustPlatform,
  makeWrapper,
  # This derivation should only be used for benchmarks-against-release task
  forBenchmarks ? true,
  _unused ? forBenchmarks,
}:
rustPlatform.buildRustPackage {
  pname = "jrsonnet";
  version = "release";

  src = fetchFromGitHub {
    owner = "CertainLach";
    repo = "jrsonnet";
    rev = "ad68a2495da324ce7a893992a6b32851849c64eb";
    hash = "sha256-N2z0JcJG6iQ+eAE1GGF+c1+T7Pti8oCgx+QWdhT+33M=";
  };
  cargoHash = "sha256-pDZSb8otrshsqdoMg7o7olEYvvjpeZ9mTaT7wN11ZWo=";

  cargoTestFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];
  cargoBuildFlags = [ "--package=jrsonnet --features=mimalloc,legacy-this-file" ];

  buildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
