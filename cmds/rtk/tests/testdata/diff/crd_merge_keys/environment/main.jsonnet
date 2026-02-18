// Test: CRD with custom merge keys from OpenAPI schema
//
// Scenario:
// - Cluster has a CRD (DatabaseCluster) with x-kubernetes-list-map-keys on spec.instances
// - Current state: two instances (primary, replica) with replicas=1,2 and storage=10Gi
// - Desired state: replica instance changed to replicas=3 and storage=20Gi
//
// Expected: diff should show only the replica instance's field changes,
// using the "name" merge key to identify which instance changed
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
    'my-db': {
      apiVersion: 'example.com/v1',
      kind: 'DatabaseCluster',
      metadata: {
        name: 'my-db',
        namespace: 'default',
      },
      spec: {
        instances: [
          {
            name: 'primary',
            replicas: 1,
            storage: '10Gi',
          },
          {
            name: 'replica',
            replicas: 3,      // Changed from 2
            storage: '20Gi',  // Changed from 10Gi
          },
        ],
      },
    },
  },
}
