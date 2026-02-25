local kustomize = std.native('kustomizeBuild');

{
  kustomizeOutput: kustomize('./kustomize', {}),
}
