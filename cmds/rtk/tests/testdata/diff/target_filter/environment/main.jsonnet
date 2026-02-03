{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'target-filter-test',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'default',
  },
  data: {
    configmap: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'my-config',
        namespace: 'default',
      },
      data: {
        key: 'value',
      },
    },
    secret: {
      apiVersion: 'v1',
      kind: 'Secret',
      metadata: {
        name: 'my-secret',
        namespace: 'default',
      },
      stringData: {
        password: 'secret123',
      },
    },
  },
}
