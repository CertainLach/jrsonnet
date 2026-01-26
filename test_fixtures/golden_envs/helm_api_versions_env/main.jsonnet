// Test case: helmTemplate with apiVersions option
// This tests that the apiVersions option is passed to helm via --api-versions flag.
// When apiVersions includes 'apps/v1/Deployment', the chart should render with apps/v1.
// Without it, the chart falls back to extensions/v1beta1.

local helmResources = std.native('helmTemplate')(
  'myrelease',
  './charts/test-chart',
  {
    calledFrom: std.thisFile,
    namespace: 'default',
    // This is the key option being tested - it should add --api-versions flags to helm
    apiVersions: ['v1', 'apps/v1', 'apps/v1/Deployment'],
  }
);

// Inline environment with helm resources
{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'helm-api-versions-test',
  },
  spec: {
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'default',
  },
  data: helmResources,
}
