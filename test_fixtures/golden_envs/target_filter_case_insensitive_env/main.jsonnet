// Test environment for case-insensitive --target flag
// Tanka's --target is case insensitive, so "configmap/.*" should match "ConfigMap"

local env = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'target-filter-case-insensitive-test',
  },
  spec: {
    apiServer: 'https://localhost:6443',
    namespace: 'default',
  },
  data: {
    // This should be exported (matches configmap/.* case-insensitively)
    'app-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'app-config',
        namespace: 'default',
      },
      data: {
        key: 'value',
      },
    },
    // This should NOT be exported (Deployment doesn't match configmap/.*)
    'app-deployment': {
      apiVersion: 'apps/v1',
      kind: 'Deployment',
      metadata: {
        name: 'app',
        namespace: 'default',
      },
      spec: {
        replicas: 1,
        selector: {
          matchLabels: { app: 'app' },
        },
        template: {
          metadata: { labels: { app: 'app' } },
          spec: {
            containers: [{
              name: 'main',
              image: 'nginx:1.25',
            }],
          },
        },
      },
    },
  },
};

env
