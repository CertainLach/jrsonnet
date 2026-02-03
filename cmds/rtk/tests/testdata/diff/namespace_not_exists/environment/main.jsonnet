{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'ns-not-exists-test',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'new-namespace',
  },
  data: {
    configmap: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'app-config',
        namespace: 'new-namespace',
      },
      data: {
        key: 'value',
      },
    },
  },
}
