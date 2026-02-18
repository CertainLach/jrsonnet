// Test: $deleteFromPrimitiveList directive for primitive arrays
//
// Scenario:
// - Original (last-applied): Pod with args ["--verbose", "--old-flag"]
// - Modified (this manifest): Pod with only ["--verbose"]
// - Current (cluster): Pod with ["--verbose", "--old-flag"]
//
// Expected: diff should show --old-flag being removed via $deleteFromPrimitiveList
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
            args: [
              '--verbose',
              // --old-flag intentionally removed
            ],
          },
        ],
      },
    },
  },
}
