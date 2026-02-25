{
  helmRelease: std.native('helmTemplate')('myrelease', './charts/my-chart', {
    values: {
      replicas: 3,
    },
  }),
  kustomizeOutput: std.native('kustomizeBuild')('./kustomize', {}),
}
