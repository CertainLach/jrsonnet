{
  // Modified resource
  modified_config: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'modified-config',
      namespace: 'default',
    },
    data: {
      key: 'new-value',
    },
  },
  // Added resource
  new_config: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'new-config',
      namespace: 'default',
    },
    data: {
      key: 'value',
    },
  },
  // Unchanged resource
  unchanged_config: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'unchanged-config',
      namespace: 'default',
    },
    data: {
      key: 'same',
    },
  },
}
