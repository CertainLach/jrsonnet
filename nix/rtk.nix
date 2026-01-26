{
  lib,
  craneLib,
  kustomize,
  kubernetes-helm,
}: let
  testInputs = [kustomize kubernetes-helm];
in
  craneLib.buildPackage {
    src = lib.cleanSourceWith {
      src = ../.;
      filter = path: type:
        (lib.hasInfix "/test_fixtures/" path)
        || (lib.hasInfix "/cmds/rtk/testdata/" path)
        || (lib.hasSuffix "\.jsonnet" path)
        || (lib.hasSuffix "\.json" path)
        || (lib.hasSuffix "\.yaml" path)
        || (lib.hasSuffix "\.yml" path)
        || (lib.hasSuffix "\.golden" path)
        || (lib.hasSuffix "\.conf" path)
        || (craneLib.filterCargoSources path type);
    };
    pname = "rtk";
    version = "0.1.0";

    # Test dependencies
    nativeBuildInputs = testInputs;
    checkInputs = testInputs;

    cargoExtraArgs = "--locked -p rtk";

    passthru = {
      inherit testInputs;
    };
  }
