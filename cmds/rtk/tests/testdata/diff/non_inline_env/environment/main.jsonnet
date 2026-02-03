// Non-inline environment: manifests are returned directly, not wrapped in Environment
{
  configmap: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'static-config',
      namespace: 'default',
    },
    data: {
      setting: 'production',
    },
  },
}
