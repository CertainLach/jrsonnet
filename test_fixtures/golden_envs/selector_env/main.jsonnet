// Test environment for --selector flag
// This tests that label selector filtering works correctly
// Only environments matching the selector (env=prod) should be exported

local makeEnv(name, envLabel) = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: name,
    labels: {
      env: envLabel,
      team: 'platform',
    },
  },
  spec: {
    apiServer: 'https://localhost:6443',
    namespace: 'default',
  },
  data: {
    config: {
      apiVersion: 'v1',
      kind: 'ConfigMap',
      metadata: {
        name: name + '-config',
        namespace: 'default',
      },
      data: {
        environment: envLabel,
      },
    },
  },
};

{
  dev: makeEnv('selector-dev', 'dev'),
  prod: makeEnv('selector-prod', 'prod'),
  staging: makeEnv('selector-staging', 'staging'),
}
