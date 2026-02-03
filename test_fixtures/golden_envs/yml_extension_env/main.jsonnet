// Test environment for --extension yml
// This tests that extension only changes the filename, not the output format (always YAML)

local env = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'yml-extension-test',
  },
  spec: {
    apiServer: 'https://localhost:6443',
    namespace: 'default',
  },
  data: {
    'yml-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'yml-config',
        namespace: 'default',
      },
      data: {
        key1: 'value1',
        key2: 'value2',
      },
    },
  },
};

env
