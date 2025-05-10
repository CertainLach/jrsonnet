{
  lib,
  craneLib,
  makeWrapper,
  withNightlyFeatures ? false,
  withExperimentalFeatures ? false,
  forBenchmarks ? false,
}:
with lib;
craneLib.buildPackage {
  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type: (lib.hasSuffix "\.jsonnet" path) || (craneLib.filterCargoSources path type);
  };
  pname = "jrsonnet";
  version = "current${optionalString withNightlyFeatures "-nightly"}${optionalString withExperimentalFeatures "-experimental"}";

  cargoExtraArgs = "--locked --features=mimalloc${optionalString withNightlyFeatures ",nightly"}${optionalString withExperimentalFeatures ",experimental"}";

  nativeBuildInputs = [ makeWrapper ];

  # To clean-up hyperfine output
  postInstall = optionalString forBenchmarks ''
    wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
  '';
}
