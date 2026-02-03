{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'prune-test',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'default',
    injectLabels: true,
  },
  data: {
    // Only this resource is in the desired state
    managed: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'managed-config',
        namespace: 'default',
      },
      data: {
        key: 'value',
      },
    },
  },
}
