{
  // Single resource - used to create initial file, then test conflict with second export
  deployment: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'test-deployment',
      namespace: 'default',
    },
  },
}
