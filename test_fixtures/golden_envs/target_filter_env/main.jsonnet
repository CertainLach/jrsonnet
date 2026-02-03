// Test environment for --target flag
// This tests that resource filtering by kind/name works correctly
// Only ConfigMap resources should be exported (matching "ConfigMap/.*")

local env = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'target-filter-test',
  },
  spec: {
    apiServer: 'https://localhost:6443',
    namespace: 'default',
  },
  data: {
    // This should be exported (matches ConfigMap/.*)
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
    // This should be exported (matches ConfigMap/.*)
    'db-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'db-config',
        namespace: 'default',
      },
      data: {
        host: 'localhost',
      },
    },
    // This should NOT be exported (Deployment doesn't match ConfigMap/.*)
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
    // This should NOT be exported (Service doesn't match ConfigMap/.*)
    'app-service': {
      apiVersion: 'v1',
      kind: 'Service',
      metadata: {
        name: 'app-service',
        namespace: 'default',
      },
      spec: {
        selector: { app: 'app' },
        ports: [{ port: 80 }],
      },
    },
  },
};

env
