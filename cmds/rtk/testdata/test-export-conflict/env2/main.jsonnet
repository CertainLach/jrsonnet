{
  // Resource that maps to the same path as env1's deployment - will cause conflict
  deployment: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'test-deployment',  // Same name as env1 - will cause conflict
      namespace: 'default',     // Same namespace as env1 - will cause conflict
    },
  },
}
