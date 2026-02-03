// This file is a function with default arguments
// It should be callable without providing TLAs
function(mode='default', replicas=1) {
  deployment: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'tla-defaults-deployment',
      namespace: 'tla-test',
    },
    spec: {
      replicas: replicas,
      selector: {
        matchLabels: {
          mode: mode,
        },
      },
    },
  },
  configmap: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'tla-defaults-config',
      namespace: 'tla-test',
    },
    data: {
      mode: mode,
    },
  },
}
