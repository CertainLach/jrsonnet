{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: { name: 'inline-env' },
  spec: { namespace: 'default' },
  data: {
    cm: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: { name: 'inline-config' },
      data: { env: 'inline' },
    },
  },
}
