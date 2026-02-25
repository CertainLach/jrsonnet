local helm = std.native('helmTemplate');

{
  helmRelease: helm('myrelease', './charts/my-chart', {
    values: {
      replicas: 3,
    },
  }),
}
