{
  lib,
  craneLib,
  cargoArtifacts,
}:
craneLib.buildPackage {
  inherit cargoArtifacts;

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      (lib.hasSuffix ".jsonnet" path)
      || (lib.hasSuffix ".toml" path)
      || (craneLib.filterCargoSources path type);
  };
  pname = "tk-compare";
  version = "0.1.0";

  cargoExtraArgs = "--locked -p tk-compare";
}
