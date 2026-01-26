// Test case for YAML multiple merge keys
// When two << merge keys are in the same mapping, both should be merged
// Expected: memcached should have fields from both memcachedClientConfig and tlsConfig

local values = std.parseYaml(importstr 'values.yaml');

{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'yaml-multiple-merge-keys-test',
  },
  spec: {
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'default',
  },
  data: {
    configmap_test: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'test-multiple-merge-keys',
        namespace: 'default',
      },
      data: {
        // Serialize the parsed memcached config to show merge key expansion
        'memcached-config': std.manifestJsonEx(values.memcached, '  '),
      },
    },
  },
}
