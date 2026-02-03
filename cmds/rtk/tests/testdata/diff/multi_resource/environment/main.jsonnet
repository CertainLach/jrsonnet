{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'test-env',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'default',
  },
  data: {
    configmap: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'app-config',
        namespace: 'default',
      },
      data: {
        setting: 'enabled',
      },
    },
    secret: {
      apiVersion: 'v1',
      kind: 'Secret',
      metadata: {
        name: 'app-secret',
        namespace: 'default',
      },
      type: 'Opaque',
      data: {
        password: 'cGFzc3dvcmQ=',
      },
    },
  },
}
