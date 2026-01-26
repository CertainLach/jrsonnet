local clusters = ['dev-region-0'];
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

// Force evaluation of data by accessing it
{
  ['env-' + cluster]: makeEnv(cluster)
  for cluster in clusters
}['env-dev-region-0'].data
