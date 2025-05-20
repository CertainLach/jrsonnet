# This derivation uses released sjsonnet binary, which most users will use
# However, recommended way of using sjsonnet - is using a client-server model,
# for which there is no released binaries: https://github.com/databricks/sjsonnet/issues/51
# TODO: Somehow build client-server version of sjsonnet, and use it in benchmarks
{
  stdenv,
  fetchurl,
  jdk23_headless,
  makeWrapper,
  java ? jdk23_headless,
}: let
  pname = "sjsonnet";
  version = "0.5.0";
in
  stdenv.mkDerivation {
    inherit pname version;

    src = fetchurl {
      url = "https://github.com/databricks/${pname}/releases/download/${version}/${pname}-${version}.jar";
      hash = "sha256-cWTpU7slHI+JsC8xfGEHLSwLMJ+U7enpBhphvbcG9us=";
    };

    unpackPhase = "true";
    buildInputs = [
      java
      makeWrapper
    ];

    installPhase = ''
      mkdir -p $out/bin $out/lib
      cp $src $out/lib/sjsonnet.jar
      makeWrapper ${java}/bin/java $out/bin/sjsonnet --add-flags "-Xss100m -XX:+UseStringDeduplication -jar $out/lib/sjsonnet.jar"
    '';
    separateDebugInfo = false;
  }
