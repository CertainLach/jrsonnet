# This derivation uses released sjsonnet binary, which most users will use
# However, recommended way of using sjsonnet - is using a client-server model,
# for which there is no released binaries: https://github.com/databricks/sjsonnet/issues/51
# TODO: Somehow build client-server version of sjsonnet, and use it in benchmarks
{
  stdenv,
  fetchurl,
  jdk20,
  makeWrapper,
}:
stdenv.mkDerivation rec {
  pname = "sjsonnet";
  version = "0.4.9";

  src = fetchurl {
    url = "https://github.com/databricks/${pname}/releases/download/${version}/${pname}-${version}.jar";
    hash = "sha256-2W5qhe+MFsHQFlbPr7RQO4mx9qRjdfpI1z4qQGcVnYM=";
  };

  unpackPhase = "true";
  buildInputs = [jdk20 makeWrapper];

  installPhase = ''
    mkdir -p $out/bin $out/lib
    cp $src $out/lib/sjsonnet.jar
    makeWrapper ${jdk20}/bin/java $out/bin/sjsonnet --add-flags "-Xss100m -XX:+UseStringDeduplication -jar $out/lib/sjsonnet.jar"
  '';
  separateDebugInfo = false;
}
