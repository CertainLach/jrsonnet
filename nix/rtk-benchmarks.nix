{
  lib,
  makeWrapper,
  runCommand,
  bash,
  hyperfine,
  packages,
}: let
  inherit (packages) rtk jrsonnet tanka;

  src = ./rtk-benchmarks.sh;
  binName = "rtk-benchmarks";
  runtimeDeps = [
    rtk
    tanka
    hyperfine
    jrsonnet
  ];
in
  runCommand binName
  {
    nativeBuildInputs = [makeWrapper];
    meta.mainProgram = binName;
  }
  ''
    mkdir -p $out/bin
    install -m 755 ${src} $out/bin/${binName}

    # Rewrite the shebang to use the pinned bash
    substituteInPlace $out/bin/${binName} \
      --replace-fail '#!/usr/bin/env bash' '#!${bash}/bin/bash'

    wrapProgram $out/bin/${binName} \
      --prefix PATH : ${lib.makeBinPath runtimeDeps} \
      --set RTK_VERSION "${rtk.version}"
  ''
