{ stdenv, jrsonnet, go-jsonnet, sjsonnet, jsonnet, hyperfine }:

stdenv.mkDerivation {
  name = "benchmarks";
  __impure = true;
  unpackPhase = "true";

  installPhase = "touch $out";
}
