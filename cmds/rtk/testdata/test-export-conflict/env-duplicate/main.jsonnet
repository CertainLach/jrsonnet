{
  // Two resources that map to the same file path - will cause conflict within same env
  deployment1: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'duplicate-deployment',
      namespace: 'default',
    },
  },
  deployment2: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'duplicate-deployment',  // Same name - will cause same file path
      namespace: 'default',          // Same namespace - will cause same file path
    },
  },
}
