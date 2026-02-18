// Test: Service ports with composite merge key (port + protocol)
//
// Scenario:
// - Original: Service with ports 80/TCP and 443/TCP
// - Modified: Update targetPort for 443/TCP, keep 80/TCP unchanged
//
// Expected: Only the 443/TCP port changes, 80/TCP remains
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
    service: {
      apiVersion: 'v1',
      kind: 'Service',
      metadata: {
        name: 'test-svc',
        namespace: 'default',
      },
      spec: {
        selector: {
          app: 'test',
        },
        ports: [
          {
            name: 'http',
            port: 80,
            protocol: 'TCP',
            targetPort: 8080,
          },
          {
            name: 'https',
            port: 443,
            protocol: 'TCP',
            targetPort: 8443,  // Changed from 443
          },
        ],
      },
    },
  },
}
