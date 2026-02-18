// Test: $retainKeys directive for volume source changes
//
// Scenario:
// - Original (last-applied): Pod with configMap volume
// - Modified (this manifest): Same volume changed to emptyDir
// - Current (cluster): Pod with configMap volume
//
// Expected: diff should show volume type change with $retainKeys
{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'test-env',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'default',
  },
  data: {
    pod: {
      apiVersion: 'v1',
      kind: 'Pod',
      metadata: {
        name: 'test-pod',
        namespace: 'default',
      },
      spec: {
        containers: [
          {
            name: 'nginx',
            image: 'nginx:1.24',
          },
        ],
        volumes: [
          {
            name: 'my-vol',
            emptyDir: {},  // Changed from configMap
          },
        ],
      },
    },
  },
}
