{
  app: {
    configs: {
      main: {
        apiVersion: 'v1',
        kind: 'ConfigMap',
        metadata: { name: 'nested-config' },
        data: { level: 'deep' },
      },
    },
  },
}
