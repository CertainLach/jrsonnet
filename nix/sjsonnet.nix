# This derivation uses released sjsonnet binary, which most users will use

# However, recommended way of using sjsonnet - is using a client-server model,
# for which there is no released binaries: https://github.com/databricks/sjsonnet/issues/51

# TODO: Somehow build client-server version of sjsonnet, and use it in benchmarks

{ stdenv, lib, fetchurl, jdk17, makeWrapper }:

stdenv.mkDerivation {
  pname = "sjsonnet";
  version = "0.4.3";

  src = fetchurl {
    url =
      "https://github.com/databricks/sjsonnet/releases/download/0.4.3/sjsonnet.jar";
    hash = "sha256-XDJAAAlFu1DfQ2YlGEO8OpWpwxzG83tHlCQIDiqfRGY=";
  };

  unpackPhase = "true";
  buildInputs =
    [ jdk17 makeWrapper ];

  installPhase = ''
    mkdir -p $out/bin $out/lib
    cp $src $out/lib/sjsonnet.jar
    makeWrapper ${jdk17}/bin/java $out/bin/sjsonnet --add-flags "-Xss100m -XX:+UseStringDeduplication -jar $out/lib/sjsonnet.jar"
  '';
  separateDebugInfo = false;
}
