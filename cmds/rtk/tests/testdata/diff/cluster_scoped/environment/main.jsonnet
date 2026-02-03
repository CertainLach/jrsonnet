{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'cluster-scoped-test',
  },
  spec: {
    contextNames: ['mock-context'],
    namespace: 'default',
  },
  data: {
    namespace: {
      apiVersion: 'v1',
      kind: 'Namespace',
      metadata: {
        name: 'my-namespace',
        labels: {
          team: 'platform',
        },
      },
    },
  },
}
