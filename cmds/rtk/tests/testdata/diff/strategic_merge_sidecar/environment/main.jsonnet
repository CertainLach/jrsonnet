// Test: Server-injected sidecar should be preserved (not deleted by diff)
//
// Scenario:
// - Original (last-applied): deployment with 'app' container
// - Modified (this manifest): deployment with 'app' container (image updated)
// - Current (cluster): deployment with 'app' + 'istio-proxy' (server-injected)
//
// Expected: diff should show only the image change, NOT try to delete istio-proxy
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
        // Note: last-applied-configuration is typically stored in cluster,
        // but rtk reads it from cluster state when doing three-way merge
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
                image: 'nginx:2.0',  // Updated from 1.0
                ports: [
                  { containerPort: 80 },
                ],
              },
            ],
          },
        },
      },
    },
  },
}
