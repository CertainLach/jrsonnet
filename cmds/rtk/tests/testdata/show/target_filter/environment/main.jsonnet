{
  cm: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: { name: 'filter-config' },
    data: { included: 'yes' },
  },
  secret: {
    apiVersion: 'v1',
    kind: 'Secret',
    metadata: { name: 'filter-secret' },
    stringData: { excluded: 'yes' },
  },
  deployment: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: { name: 'filter-deploy' },
    spec: {
      selector: { matchLabels: { app: 'test' } },
      template: {
        metadata: { labels: { app: 'test' } },
        spec: { containers: [{ name: 'app', image: 'nginx' }] },
      },
    },
  },
}
