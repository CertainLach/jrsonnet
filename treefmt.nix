{
  settings.global.excludes = [
    "*.adoc"
    "*.png"
    "*.svg"
    "*.golden"
    "*.snap"
    "*.ungram"
    ".gitignore"
    "Makefile"
    "LICENSE"
    ".editorconfig"
    ".github/hooks/pre-commit"
    "nix/benchmarks.md"

    # TODO: Use jrsonnet-fmt
    "*.jsonnet"
  ];

  programs.nixfmt.enable = true;
  programs.shfmt.enable = true;
  programs.rustfmt.enable = true;
  programs.taplo.enable = true;
  programs.yamlfmt.enable = true;
  programs.clang-format.enable = true;
}
