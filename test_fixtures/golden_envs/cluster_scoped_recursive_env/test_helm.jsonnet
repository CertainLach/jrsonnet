local helmResources = std.native('helmTemplate')(
  'flagger',
  './charts/flagger-chart',
  {
    calledFrom: std.thisFile,
    namespace: 'flagger',
    values: {
      clusterName: 'test',
    },
  }
);
helmResources
