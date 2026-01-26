// Test case: Multiple environments using helmTemplate with cluster-scoped resources
// This tests recursive export with helmTemplate-generated resources including CRDs

local clusters = ['dev-region-0', 'prod-region-3'];

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
  
  // Modify CRD name to include cluster name to avoid conflicts
  local modifiedResources = {
    [k]: if helmResources[k].kind == 'CustomResourceDefinition' && helmResources[k].metadata.name == 'canaries.flagger.app' then
      helmResources[k] {
        metadata+: {
          name: 'canaries-' + cluster + '.flagger.app',
        },
      }
    else
      helmResources[k]
    for k in std.objectFields(helmResources)
  },

  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'flagger-' + cluster,
    labels: {
      cluster: cluster,
    },
  },
  spec: {
    apiServer: 'https://' + cluster + '.example.com:6443',
    namespace: 'flagger',
  },
  data: modifiedResources,
};

// Return multiple environments for recursive export
{
  ['env-' + cluster]: makeEnv(cluster)
  for cluster in clusters
}
