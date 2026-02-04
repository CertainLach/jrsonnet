{
  apiVersion: 'v1',
  kind: 'List',
  items: [
    {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: { name: 'list-cm-1' },
      data: { source: 'list' },
    },
    {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: { name: 'list-cm-2' },
      data: { source: 'list' },
    },
  ],
}
