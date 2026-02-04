{
  configmap: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: { name: 'app-config' },
    data: { setting: 'value' },
  },
  secret: {
    apiVersion: 'v1',
    kind: 'Secret',
    metadata: { name: 'app-secret' },
    stringData: { password: 'secret123' },
  },
}
