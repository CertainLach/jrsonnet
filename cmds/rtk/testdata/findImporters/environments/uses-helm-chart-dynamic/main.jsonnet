local tanka = import 'github.com/grafana/jsonnet-libs/tanka-util/main.libsonnet';
local helm = tanka.helm.new(std.thisFile);

local chartVersion = '1.0.0';

{
  helmRelease: helm.template('myrelease', './charts/my-dynamic-chart-%s' % chartVersion, {
    values: {
      replicas: 3,
    },
  }),
}
