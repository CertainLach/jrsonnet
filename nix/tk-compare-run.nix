{
  lib,
  makeWrapper,
  runCommand,
  packages,
}: let
  inherit (packages) rtk tk-compare tanka jrsonnet;

  configTemplate = ../tk-compare-grafana.toml;
  # Source directories for test fixtures
  goldenEnvsDir = ../test_fixtures/golden_envs;
  diffFixturesDir = ../cmds/rtk/tests/testdata/diff;

  # List directories in a path
  listDirs = dir:
    lib.filterAttrs
    (_: type: type == "directory")
    (builtins.readDir dir);

  # Check if a file exists and contains a pattern
  fileContains = file: pattern:
    builtins.pathExists file && lib.hasInfix pattern (builtins.readFile file);

  # Check if env uses exportJsonnetImplementation (not supported)
  usesExportJsonnet = envDir: let
    specJson = envDir + "/spec.json";
    mainJsonnet = envDir + "/main.jsonnet";
  in
    fileContains specJson "exportJsonnetImplementation"
    || fileContains mainJsonnet "exportJsonnetImplementation";

  # Check if env is jrsonnet-specific. If it is, we don't want to run Tanka
  # against it.
  isJrsonnetSpecific = name: lib.hasSuffix "_jrsonnet" name;

  # Golden envs: filter valid test directories
  goldenEnvDirs = listDirs goldenEnvsDir;
  goldenTestNames = lib.filterAttrs (name: _:
    !usesExportJsonnet (goldenEnvsDir + "/${name}")
    && !isJrsonnetSpecific name)
  goldenEnvDirs;

  # Diff fixtures: filter valid test directories
  diffFixtureDirs = listDirs diffFixturesDir;

  isValidDiffFixture = name: let
    testDir = diffFixturesDir + "/${name}";
    envDir = testDir + "/environment";
    clusterDir = testDir + "/cluster";
    isErrorTest = lib.hasInfix "error" name || lib.hasInfix "invalid" name;
  in
    builtins.pathExists envDir
    && builtins.pathExists clusterDir
    && !isErrorTest;

  diffTestNames = lib.filterAttrs (name: _: isValidDiffFixture name) diffFixtureDirs;
  goldenCopyCommands =
    lib.concatStringsSep "\n"
    (map (name: "cp -r \"$goldenEnvsSrc/${name}\" \"$out/share/fixtures/golden_envs/${name}\"") (builtins.attrNames goldenTestNames));

  diffCopyCommands =
    lib.concatStringsSep "\n"
    (map (name: "cp -r \"$diffFixturesSrc/${name}\" \"$out/share/fixtures/diff/${name}\"") (builtins.attrNames diffTestNames));

  binName = "tk-compare-run";
  runtimeDeps = [
    rtk
    tk-compare
    tanka
    jrsonnet
  ];
in
  runCommand binName
  {
    nativeBuildInputs = [makeWrapper];
    meta.mainProgram = binName;

    goldenEnvsSrc = goldenEnvsDir;
    diffFixturesSrc = diffFixturesDir;
  }
  ''
    mkdir -p $out/bin $out/share/fixtures
    mkdir -p $out/share/fixtures/golden_envs $out/share/fixtures/diff

    # Copy only fixtures that are valid for tk-compare parity runs
    ${goldenCopyCommands}
    ${diffCopyCommands}

    # Copy and patch shared config file with Nix fixture paths
    cp ${configTemplate} $out/share/tk-compare-fixtures.toml
    substituteInPlace $out/share/tk-compare-fixtures.toml \
      --replace-fail 'test_fixtures/golden_envs' "$out/share/fixtures/golden_envs" \
      --replace-fail 'cmds/rtk/tests/testdata/diff' "$out/share/fixtures/diff"

    # Create wrapper that runs tk-compare with our config
    makeWrapper ${tk-compare}/bin/tk-compare $out/bin/${binName} \
      --prefix PATH : ${lib.makeBinPath runtimeDeps} \
      --set TK_PATH ${tanka}/bin/tk \
      --set RTK_PATH ${rtk}/bin/rtk \
      --set JRSONNET_PATH ${jrsonnet}/bin/jrsonnet \
      --add-flags "$out/share/tk-compare-fixtures.toml"
  ''
