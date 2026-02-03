// Test environment for external variables (--ext-str and --ext-code)
// This tests that external variables are properly passed through to the evaluation

local env = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'ext-vars-test',
  },
  spec: {
    apiServer: 'https://localhost:6443',
    namespace: 'default',
  },
  data: {
    'ext-vars-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'ext-vars-config',
        namespace: 'default',
      },
      data: {
        // String external variable
        'string-var': std.extVar('stringVar'),
        // Code external variable (should be evaluated as JSON)
        'code-var': std.manifestJson(std.extVar('codeVar')),
        // Combined usage
        combined: std.extVar('stringVar') + '-' + std.toString(std.extVar('codeVar').count),
      },
    },
  },
};

env
