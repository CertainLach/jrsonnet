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
    // Only one configmap in manifests - the other one in cluster should be pruned
    configmap: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'keep-this',
        namespace: 'default',
      },
      data: {
        key: 'value',
      },
    },
  },
}
