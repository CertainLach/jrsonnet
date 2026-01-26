// Test case: Multiple inline environments with helmTemplate, injectLabels, and fluxExportDir
// This tests the pattern used by flagger and other multi-cluster environments
// Key features tested:
// 1. Multiple inline environments in .envs
// 2. helmTemplate with std.thisFile (requires absolute path)
// 3. injectLabels: true for tanka.dev/environment label injection
// 4. cluster_name and fluxExportDir labels for export directory structure
// 5. Per-cluster resources (different values per environment)

// Define cluster configurations
local clusters = {
  'dev-region-3': {
    cluster_name: 'dev-region-3',
    apiServer: 'https://apk-fpglzi-3.kitwira.qgc',
  },
  'prod-region-3': {
    cluster_name: 'prod-region-3',
    apiServer: 'https://hxnp-ryhwpu-3.wpozacj.hmw',
  },
};

{
  local this = self,
  namespace:: 'flagger',
  app:: 'flagger',

  // Environment generator function - uses cluster-specific values
  environment(clusterName)::
    local cluster = clusters[clusterName];
    // Generate helm resources with cluster-specific values
    local helmResources = std.native('helmTemplate')(
      'flagger',
      './charts/flagger-chart',
      {
        calledFrom: std.thisFile,
        namespace: this.namespace,
        includeCrds: clusterName == 'dev-region-3',
        values: {
          clusterName: cluster.cluster_name,
        },
      }
    );
    {
      apiVersion: 'tanka.dev/v1alpha1',
      kind: 'Environment',
      metadata: {
        name: 'environments/%s/%s.%s' % [this.app, cluster.cluster_name, this.namespace],
        namespace: this.namespace,
        labels: {
          app: this.app,
          cluster_name: cluster.cluster_name,
          fluxExportDir: this.namespace,
          inline: 'true',
        },
      },
      spec: {
        apiServer: cluster.apiServer,
        namespace: this.namespace,
        injectLabels: true,
      },
      data: helmResources,
    },

  // Generate environments for all clusters
  envs: {
    [clusterName]: this.environment(clusterName)
    for clusterName in std.objectFields(clusters)
  },
}
