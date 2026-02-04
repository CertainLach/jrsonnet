{
  'env-a': {
    apiVersion: 'tanka.dev/v1alpha1',
    kind: 'Environment',
    metadata: { name: 'env-a' },
    spec: { namespace: 'ns-a' },
    data: {
      cm: {
        apiVersion: 'v1',
        kind: 'ConfigMap',
        metadata: { name: 'config-a' },
        data: { env: 'a' },
      },
    },
  },
  'env-b': {
    apiVersion: 'tanka.dev/v1alpha1',
    kind: 'Environment',
    metadata: { name: 'env-b' },
    spec: { namespace: 'ns-b' },
    data: {
      cm: {
        apiVersion: 'v1',
        kind: 'ConfigMap',
        metadata: { name: 'config-b' },
        data: { env: 'b' },
      },
    },
  },
}
