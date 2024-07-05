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
      filter = path: type:
      # Tests use .jsonnet files.
        (lib.hasSuffix "\.jsonnet" path)
        || (lib.hasSuffix "\.libsonnet" path)
        || (craneLib.filterCargoSources path type);
    };
    pname = "jrsonnet";
    version = "current${optionalString withNightlyFeatures "-nightly"}${optionalString withExperimentalFeatures "-experimental"}";

    cargoExtraArgs = "--locked --features=mimalloc${optionalString withNightlyFeatures ",nightly"}${optionalString withExperimentalFeatures ",experimental"}";

    env = lib.optionalAttrs withNightlyFeatures {
      # Do not panic on pipe failure: https://github.com/rust-lang/rust/issues/97889
      # https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/on-broken-pipe.html
      # FIXME: Maybe inherit should be used here?
      RUSTFLAGS = "-Zon-broken-pipe=kill";
    };

    nativeBuildInputs = [makeWrapper];

    # To clean-up hyperfine output
    postInstall = optionalString forBenchmarks ''
      wrapProgram $out/bin/jrsonnet --add-flags "--max-stack=200000 --os-stack=200000"
    '';
  }
