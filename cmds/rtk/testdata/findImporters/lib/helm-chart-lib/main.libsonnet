local tanka = import 'github.com/grafana/jsonnet-libs/tanka-util/main.libsonnet';
local helm = tanka.helm.new(std.thisFile);

{
  deploy():: helm.template('myrelease', './charts/my-lib-chart', {
    values: {
      replicas: 3,
    },
  }),
}
