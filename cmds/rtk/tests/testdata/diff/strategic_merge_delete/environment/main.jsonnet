// Test: Delete directive should be generated when removing a container
//
// Scenario:
// - Original (last-applied): deployment with 'app' and 'sidecar' containers
// - Modified (this manifest): deployment with only 'app' container
// - Current (cluster): deployment with 'app' and 'sidecar'
//
// Expected: diff should show delete directive for 'sidecar'
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
    deployment: {
      apiVersion: 'apps/v1',
      kind: 'Deployment',
      metadata: {
        name: 'web-app',
        namespace: 'default',
      },
      spec: {
        replicas: 1,
        selector: {
          matchLabels: {
            app: 'web',
          },
        },
        template: {
          metadata: {
            labels: {
              app: 'web',
            },
          },
          spec: {
            containers: [
              {
                name: 'app',
                image: 'nginx:1.0',
                ports: [
                  { containerPort: 80 },
                ],
              },
              // Note: 'sidecar' container is intentionally removed
            ],
          },
        },
      },
    },
  },
}
