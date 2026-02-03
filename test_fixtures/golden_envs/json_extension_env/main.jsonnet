// Test environment for --extension json
// This tests that JSON output format works consistently between tk and rtk

local env = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'json-extension-test',
  },
  spec: {
    apiServer: 'https://localhost:6443',
    namespace: 'default',
  },
  data: {
    'json-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'json-config',
        namespace: 'default',
      },
      data: {
        key1: 'value1',
        key2: 'value2',
        nested: std.manifestJson({
          a: 1,
          b: 'two',
          c: [1, 2, 3],
        }),
      },
    },
    'json-deployment': {
      apiVersion: 'apps/v1',
      kind: 'Deployment',
      metadata: {
        name: 'json-app',
        namespace: 'default',
      },
      spec: {
        replicas: 2,
        selector: {
          matchLabels: {
            app: 'json-app',
          },
        },
        template: {
          metadata: {
            labels: {
              app: 'json-app',
            },
          },
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
