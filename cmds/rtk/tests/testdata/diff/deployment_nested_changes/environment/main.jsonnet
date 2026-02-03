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
    deployment: {
      apiVersion: 'apps/v1',
      kind: 'Deployment',
      metadata: {
        name: 'web-app',
        namespace: 'default',
      },
      spec: {
        replicas: 3,
        selector: {
          matchLabels: {
            app: 'web',
          },
        },
        template: {
          metadata: {
            labels: {
              app: 'web',
            },
          },
          spec: {
            containers: [
              {
                name: 'nginx',
                image: 'nginx:1.25.0',
                ports: [
                  { containerPort: 80 },
                ],
                env: [
                  { name: 'LOG_LEVEL', value: 'info' },
                  { name: 'WORKERS', value: '4' },
                ],
                resources: {
                  limits: {
                    cpu: '500m',
                    memory: '256Mi',
                  },
                  requests: {
                    cpu: '100m',
                    memory: '128Mi',
                  },
                },
              },
              {
                name: 'sidecar',
                image: 'busybox:1.36',
                command: ['sh', '-c', 'while true; do echo heartbeat; sleep 30; done'],
              },
            ],
          },
        },
      },
    },
  },
}
