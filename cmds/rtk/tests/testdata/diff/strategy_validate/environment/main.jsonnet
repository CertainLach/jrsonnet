{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'validate-test',
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
        name: 'validate-config',
        namespace: 'default',
      },
      data: {
        key: 'new-value',
      },
    },
  },
}
