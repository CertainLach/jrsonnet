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
    // This ConfigMap does NOT have an explicit namespace - relies on spec.namespace
    // The cluster resource HAS the namespace set explicitly.
    // Prune should NOT delete this because it's the same resource.
    configmap: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'keep-config',
        // NOTE: No namespace here - relies on spec.namespace: 'default'
      },
      data: {
        key: 'value',
      },
    },
  },
}
