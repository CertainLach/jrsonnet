// Golden test: inline envs with mixed exportJsonnetImplementation.
// Only the env that sets exportJsonnetImplementation should get jrsonnet output formatting;
// the other env must use default (go-jsonnet) formatting.
// This verifies per-env handling when only one of multiple inline envs has the spec set.

local makeConfigMap(namespace, name, yamlDoc) = {
  apiVersion: 'v1',
  kind: 'ConfigMap',
  metadata: {
    name: name,
    namespace: namespace,
  },
  data: {
    'config.yaml': yamlDoc,
  },
};

// Shared data passed to std.manifestYamlDoc - output format differs by jrsonnet vs go-jsonnet
local sampleYamlData = {
  key: 'value %s' % 1.1,
  num: 1,
  list: [1.1, 2.2],
};

// Inline env WITH exportJsonnetImplementation -> jrsonnet formatting for std.manifestYamlDoc
local envWithJrsonnet = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: { name: 'with-jrsonnet' },
  spec: {
    namespace: 'with-jrsonnet',
    apiServer: 'https://example.com',
    exportJsonnetImplementation: 'binary:/usr/local/bin/jrsonnet',
  },
  data: {
    'ConfigMap-rules': makeConfigMap(
      'with-jrsonnet',
      'rules',
      std.manifestYamlDoc(sampleYamlData)
    ),
  },
};

// Inline env WITHOUT exportJsonnetImplementation -> default (go-jsonnet) formatting
local envWithoutJrsonnet = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: { name: 'without-jrsonnet' },
  spec: {
    namespace: 'without-jrsonnet',
    apiServer: 'https://example.com',
  },
  data: {
    'ConfigMap-rules': makeConfigMap(
      'without-jrsonnet',
      'rules',
      std.manifestYamlDoc(sampleYamlData)
    ),
  },
};

{
  with_jrsonnet: envWithJrsonnet,
  without_jrsonnet: envWithoutJrsonnet,
}
