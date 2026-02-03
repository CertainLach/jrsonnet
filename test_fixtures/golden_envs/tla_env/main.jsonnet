// Test environment for top-level arguments (--tla-str and --tla-code)
// This tests that TLAs are properly passed through to the evaluation
// The main.jsonnet must be a function to accept TLAs

function(stringArg, codeArg) {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'tla-test',
  },
  spec: {
    apiServer: 'https://localhost:6443',
    namespace: 'default',
  },
  data: {
    'tla-config': {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: 'tla-config',
        namespace: 'default',
      },
      data: {
        // String TLA
        'string-arg': stringArg,
        // Code TLA (should be evaluated as JSON)
        'code-arg': std.manifestJson(codeArg),
        // Combined usage
        combined: stringArg + '-' + std.toString(codeArg.value),
      },
    },
  },
}
