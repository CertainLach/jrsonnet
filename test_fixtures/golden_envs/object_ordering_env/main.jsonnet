// Test cases for object key ordering and field serialization differences between tk and rtk
// Issues reproduced from tk-compare comparison:
// 1. Object key ordering in ConfigMaps (affects config hashes)
// 2. Numeric string key ordering (10,67,100 vs 10,100,67)

// Test case 1: Object with numeric string keys that sort differently
// as strings vs numbers. tk appears to sort as strings, rtk as numbers.
// "67" > "100" as strings, but 67 < 100 as numbers
local endpointMapping = {
  '10': 'http://aealzzh-10.skafi/bexn',
  '100': 'http://qdxcpoy-100.lqsdo/qhtb',
  '105': 'http://xzvqjou-105.abpmn/vdpw',
  '132': 'http://occktec-132.eitus/rwkm',
  '188': 'http://tqjoqtn-188.dzwdu/uyrx',
  '67': 'http://gnohnko-67.mnxdk/nkvi',
};

// Build a comma-separated list from the object keys
// This simulates how metrics-endpoint-list is built in the real codebase
local buildEndpointList(mapping) =
  std.join(',', [
    '%s:%s' % [k, mapping[k]]
    for k in std.objectFields(mapping)
  ]);

// Build a comma-separated list of just the keys
local buildIdList(mapping) =
  std.join(',', std.objectFields(mapping));

// Test case 2: Nested object ordering - keys at multiple levels
local nestedConfig = {
  servers: {
    'server-200': { port: 8080 },
    'server-30': { port: 8081 },
    'server-5': { port: 8082 },
  },
  routes: {
    '/api/v2': { handler: 'v2' },
    '/api/v10': { handler: 'v10' },
    '/api/v1': { handler: 'v1' },
  },
};

// Test case 3: Array of objects with numeric-string keys
local arrayWithNumericKeys = [
  { id: '100', name: 'item-100' },
  { id: '20', name: 'item-20' },
  { id: '3', name: 'item-3' },
];

{
  // Deployment with args that depend on object field ordering
  'metrics-mapper-deployment': {
    assert self.apiVersion == 'apps/v1' : 'must use apps/v1',
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      assert std.length(self.name) > 0 : 'name is required',
      name: 'metrics-instance-mapper',
      namespace: 'default',
    },
    spec: {
      template: {
        spec: {
          containers: [{
            assert std.isString(self.image) : 'image must be string',
            assert std.length(self.args) == 2 : 'should have exactly 2 args',
            name: 'mapper',
            image: 'mapper:latest',
            args: [
              // This arg's value depends on object field iteration order
              '-proxy.metrics-endpoint-list=' + buildEndpointList(endpointMapping),
              // This arg's value also depends on object field iteration order
              '-proxy.valid-metrics-cluster-id=' + buildIdList(endpointMapping),
            ],
          }],
        },
      },
    },
  },
  // ConfigMap that will have different hash due to key ordering
  'config-with-ordering': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'config-ordering-test',
      namespace: 'default',
    },
    data: {
      'config.yaml': std.manifestYamlDoc(nestedConfig),
      'endpoints.txt': buildEndpointList(endpointMapping),
      'ids.txt': buildIdList(endpointMapping),
    },
  },
  // Test sorting in manifestYamlDoc output
  'array-ordering-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'array-ordering',
      namespace: 'default',
    },
    data: {
      // Arrays should maintain insertion order, not be sorted
      'items.yaml': std.manifestYamlDoc({
        items: arrayWithNumericKeys,
      }),
    },
  },
}
