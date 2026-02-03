{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'deployment-test',
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
            containers: [{
              name: 'nginx',
              image: 'nginx:1.25',
            }],
          },
        },
      },
    },
  },
}
