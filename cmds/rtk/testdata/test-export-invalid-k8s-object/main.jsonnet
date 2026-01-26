{
  // Valid k8s object
  configmap: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'valid-config',
    },
    data: {
      key: 'value',
    },
  },
  // Invalid k8s object - has kind and metadata but missing apiVersion
  thor_engine: {
    kind: 'ConfigMap',
    metadata: {
      name: 'invalid-config',
    },
    data: {
      key: 'value',
    },
  },
}
