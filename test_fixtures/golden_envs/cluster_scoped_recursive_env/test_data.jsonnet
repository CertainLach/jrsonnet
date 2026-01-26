local makeEnv(cluster) = {
  local helmResources = std.native('helmTemplate')(
    'flagger',
    './charts/flagger-chart',
    {
      calledFrom: std.thisFile,
      namespace: 'flagger',
      values: {
        clusterName: cluster,
      },
    }
  ),

  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  data: helmResources,
};
makeEnv('test').data
