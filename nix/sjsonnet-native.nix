{
  stdenv,
  fetchurl,
}: let
  pname = "sjsonnet";
  version = "0.5.0";
in
  stdenv.mkDerivation {
    inherit pname version;

    # TODO: Add support for aarch64-darwin
    # FIXME: Mark as unsupported on other architectures
    src = fetchurl {
      url = "https://github.com/databricks/${pname}/releases/download/${version}/${pname}-${version}-Linux-x86_64";
      hash = "sha256-eCd38pe4T58DMkFmftXpUn9HdasaMkQzIe5T2g77lSs=";
    };

    unpackPhase = "true";
    installPhase = ''
      mkdir -p $out/bin
      cp $src $out/bin/sjsonnet-native
      chmod +x $out/bin/sjsonnet-native
    '';
  }
