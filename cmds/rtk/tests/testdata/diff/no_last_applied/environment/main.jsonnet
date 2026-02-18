// Test: No deletions when last-applied-configuration is missing
//
// Scenario:
// - Cluster: Deployment with 2 containers (no last-applied annotation)
// - Modified (this manifest): Deployment with 1 container
//
// Expected: No deletions - we can't know what was user-applied vs server-injected
// The diff should only show changes to the "app" container
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
        name: 'test-deploy',
        namespace: 'default',
      },
      spec: {
        replicas: 1,
        selector: {
          matchLabels: {
            app: 'test',
          },
        },
        template: {
          metadata: {
            labels: {
              app: 'test',
            },
          },
          spec: {
            containers: [
              {
                name: 'app',
                image: 'nginx:1.25',  // Updated from 1.24
              },
            ],
          },
        },
      },
    },
  },
}
