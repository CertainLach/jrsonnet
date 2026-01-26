// Test case for YAML merge key expansion bug
// When std.parseYaml reads YAML with anchors and merge keys (<<),
// the merge keys should be expanded, not preserved as literal keys.
//
// This tests the fix for: serde_yaml_with_quirks not expanding YAML merge keys

local values = std.parseYaml(importstr 'values.yaml');

// Return the parsed values directly to verify merge key expansion
// If working correctly:
//   - deployment.labels should have {app: "test-app", version: "1.0", component: "backend"}
//   - deployment.volumes[0] should have {name: "cert-secret-volume", secret: {...}}
// If broken:
//   - deployment.labels will have {"<<": {...}, component: "backend"}
//   - deployment.volumes[0] will have {"<<": {...}, secret: {...}}

{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'yaml-merge-key-test',
  },
  spec: {
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'default',
  },
  data: {
    // Wrap in a ConfigMap so export works
    configmap_test_values: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'test-values',
        namespace: 'default',
      },
      data: {
        // Serialize the parsed values to show merge key expansion
        'parsed-labels': std.manifestJsonEx(values.deployment.labels, '  '),
        'parsed-volumes': std.manifestJsonEx(values.deployment.volumes, '  '),
      },
    },
  },
}
