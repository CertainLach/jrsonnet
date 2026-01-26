// Test case: YAML 1.1 octal number parsing in helmTemplate
// Go yaml.v3 interprets 0755 as octal (493 decimal)
// serde-saphyr currently only handles 00755 double-zero prefix
// This test should output defaultMode: 493 (not 755)

local helmResources = std.native('helmTemplate')(
  'octal-test',
  './charts/octal-chart',
  {
    calledFrom: std.thisFile,
    namespace: 'default',
    values: {},
  }
);

helmResources
