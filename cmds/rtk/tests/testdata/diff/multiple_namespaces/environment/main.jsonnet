{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'test-env',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'production',
  },
  data: {
    // Production namespace resources
    'prod-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'app-config',
        namespace: 'production',
      },
      data: {
        env: 'production',
        replicas: '3',
      },
    },
    'prod-secret': {
      apiVersion: 'v1',
      kind: 'Secret',
      metadata: {
        name: 'db-credentials',
        namespace: 'production',
      },
      type: 'Opaque',
      data: {
        password: 'bmV3cGFzc3dvcmQ=',
      },
    },
    // Staging namespace resources
    'staging-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'app-config',
        namespace: 'staging',
      },
      data: {
        env: 'staging',
        replicas: '1',
      },
    },
    // Monitoring namespace resource (new)
    'monitoring-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'prometheus-config',
        namespace: 'monitoring',
      },
      data: {
        scrapeInterval: '30s',
      },
    },
  },
}
