// Test case: helmTemplate with CRD containing YAML merge keys (!!merge <<: *anchor)
// This reproduces the issue seen with clickhouse-operator CRDs where merge keys
// are not being expanded by the YAML parser.

local helmResources = std.native('helmTemplate')(
  'myrelease',
  './charts/test-chart',
  {
    calledFrom: std.thisFile,
    namespace: 'default',
    includeCrds: true,
  }
);

// Inline environment with helm resources
{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'helm-merge-key-crd-test',
  },
  spec: {
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'default',
  },
  data: helmResources,
}
