{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'inject-labels-test',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'default',
    injectLabels: true,
  },
  data: {
    configmap: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'needs-label',
        namespace: 'default',
      },
      data: {
        key: 'value',
      },
    },
  },
}
