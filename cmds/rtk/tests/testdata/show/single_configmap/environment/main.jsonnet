{
  apiVersion: 'v1',
  kind: 'ConfigMap',
  metadata: {
    name: 'test-config',
    namespace: 'default',
  },
  data: {
    key: 'value',
    another: 'entry',
  },
}
