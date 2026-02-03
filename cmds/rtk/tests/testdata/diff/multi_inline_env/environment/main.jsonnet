// Multiple inline environments - requires --name to select one
{
  'env-a': {
    apiVersion: 'tanka.dev/v1alpha1',
    kind: 'Environment',
    metadata: {
      name: 'env-a',
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
          name: 'env-a-config',
          namespace: 'default',
        },
        data: {
          key: 'value-a',
        },
      },
    },
  },
  'env-b': {
    apiVersion: 'tanka.dev/v1alpha1',
    kind: 'Environment',
    metadata: {
      name: 'env-b',
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
          name: 'env-b-config',
          namespace: 'default',
        },
        data: {
          key: 'value-b',
        },
      },
    },
  },
}
