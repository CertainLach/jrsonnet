// Test case: helmTemplate with inline Environment and resourceDefaults
// This tests whether annotations from resourceDefaults are applied to helm-generated resources

local withFluxIgnore(ignoredBy, ignoredReason='') = {
  spec+: {
    resourceDefaults+: {
      annotations+: {
        'kustomize.toolkit.fluxcd.io/reconcile': 'disabled',
        'kustomize.toolkit.fluxcd.io/reconcile-disabled-by': ignoredBy,
        'kustomize.toolkit.fluxcd.io/reconcile-disabled-reason': 'Ignored with the withFluxIgnore jsonnet utility. ' + ignoredReason,
      },
    },
  },
};

// Render the helm chart
local helmResources = std.native('helmTemplate')(
  'myrelease',
  './charts/test-chart',
  {
    calledFrom: std.thisFile,
    namespace: 'default',
    values: {
      replicaCount: 2,
      image: {
        repository: 'nginx',
        tag: '1.25',
      },
      service: {
        type: 'ClusterIP',
        port: 8080,
      },
    },
  }
);

// Inline environment with helm resources and resourceDefaults
local env = {
  assert self.kind == 'Environment' : 'must be Environment kind',
  assert self.apiVersion == 'tanka.dev/v1alpha1' : 'must use tanka.dev/v1alpha1',
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    assert std.isString(self.name) : 'metadata.name must be string',
    assert std.objectHas(self.labels, 'cluster') : 'must have cluster label',
    name: 'helm-test',
    labels: {
      cluster: 'test-cluster',
      fluxExport: 'false',
    },
  },
  spec: {
    assert std.startsWith(self.apiServer, 'https://') : 'apiServer must use https',
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'default',
  },
  data: helmResources {
    deployment_test_k_8s_deployment+: {
      hello: 'world',
    },

    job_flux2_flux_check_o_11y_ingest+: {
      hello: 'world',
    },
  },
} + withFluxIgnore('platform-federal', 'Flux not running in federal clusters');

{
  nested: {
    nestedAgain: env,
  },
}
