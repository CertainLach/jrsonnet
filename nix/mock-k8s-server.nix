{
  lib,
  craneLib,
  cargoArtifacts,
}:
craneLib.buildPackage {
  inherit cargoArtifacts;

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type: (craneLib.filterCargoSources path type);
  };
  pname = "mock-k8s-server";
  version = "0.1.0";

  cargoExtraArgs = "--locked -p mock-k8s-server";
}
