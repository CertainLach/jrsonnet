# This derivation uses released sjsonnet binary, which most users will use
# However, recommended way of using sjsonnet - is using a client-server model,
# for which there is no released binaries: https://github.com/databricks/sjsonnet/issues/51
# TODO: Somehow build client-server version of sjsonnet, and use it in benchmarks
{
  stdenv,
  lib,
  fetchurl,
  jdk25_headless,
  makeWrapper,
  autoPatchelfHook,
  zlib,
  openssl,
  java ? jdk25_headless,
}:
let
  version = "0.6.2";
  baseUrl = "https://github.com/databricks/sjsonnet/releases/download/${version}";

  nativePlatform = {
    x86_64-linux = "linux-x86_64";
    aarch64-linux = "linux-arm64";
    aarch64-darwin = "darwin-arm64";
    # Nobody cares about darwin on intel
  }.${stdenv.hostPlatform.system} or (throw "unsupported system: ${stdenv.hostPlatform.system}");

  nativeSrc = fetchurl {
    url = "${baseUrl}/sjsonnet-${version}-${nativePlatform}";
    hash = {
      x86_64-linux = "sha256-r79Q6SovcPIomDDUYTGIP35/y5t9Xo5Z3ohP7pxsF8I=";
      aarch64-linux = "sha256-LxWR94u1Oncau57Kmtzj2UFEofWGT4+laDTbRDkwv08=";
      aarch64-darwin = "sha256-gNZ6XekMm+ebeD7UFeRfoapXw/90gHk8MoeGD86dzKk=";
    }.${stdenv.hostPlatform.system};
  };

  graalvmSrc = fetchurl {
    url = "${baseUrl}/sjsonnet-graalvm-${version}-${nativePlatform}";
    hash = {
      x86_64-linux = "sha256-XNJCnQlwVFySrUevn1nLN/DY8UBGgvCFAitkedLB+yM=";
      aarch64-linux = "sha256-J+C3pmiBEmo8M00IDHp9jznonibXmHVRlhsvQ4apVWw=";
      aarch64-darwin = "sha256-TXNL52sS4NE3GjbOFtgf6aCP268qGKMFaoTwAfJfgGc=";
    }.${stdenv.hostPlatform.system};
  };
in
stdenv.mkDerivation {
  pname = "sjsonnet";
  inherit version;

  src = fetchurl {
    url = "${baseUrl}/sjsonnet-${version}.jar";
    hash = "sha256-jvkrxY48d/UdMGRoB9SxCJU2JJirfhZNyNMSkMrrQ7w=";
  };

  unpackPhase = "true";
  nativeBuildInputs = [ makeWrapper ]
    ++ lib.optionals stdenv.hostPlatform.isLinux [ autoPatchelfHook ];
  buildInputs = [ java ]
    ++ lib.optionals stdenv.hostPlatform.isLinux [ zlib openssl stdenv.cc.cc.lib ];

  installPhase = ''
    mkdir -p $out/bin $out/lib
    cp $src $out/lib/sjsonnet.jar
    makeWrapper ${java}/bin/java $out/bin/sjsonnet --add-flags "-Xss100m -XX:+UseStringDeduplication -jar $out/lib/sjsonnet.jar"

    cp ${nativeSrc} $out/bin/sjsonnet-native
    chmod +x $out/bin/sjsonnet-native
    wrapProgram $out/bin/sjsonnet-native --add-flags "--max-stack 200000"

    cp ${graalvmSrc} $out/bin/sjsonnet-graalvm
    chmod +x $out/bin/sjsonnet-graalvm
    wrapProgram $out/bin/sjsonnet-graalvm --add-flags "--max-stack 200000"
  '';
  separateDebugInfo = false;
}
