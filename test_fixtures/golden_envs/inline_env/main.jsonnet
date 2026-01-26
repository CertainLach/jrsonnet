// Test inline environment - defines Tanka Environment objects directly in Jsonnet
// This tests the inline environment discovery and export code paths

local withFluxIgnore(ignoredBy, ignoredReason='') = {
  spec+: {
    resourceDefaults+: {
      annotations+: {
        'kustomize.toolkit.fluxcd.io/reconcile': 'disabled',
        'kustomize.toolkit.fluxcd.io/reconcile-disabled-by': ignoredBy,
        'kustomize.toolkit.fluxcd.io/reconcile-disabled-reason': 'Ignored with the withFluxIgnore jsonnet utility. ' + ignoredReason,
      },
    },
  },
};

local makeConfigMap(name, data) = {
  apiVersion: 'v1',
  kind: 'ConfigMap',
  metadata: {
    name: name,
    namespace: 'default',
  },
  data: data,
};

local makeDeployment(name, image) = {
  apiVersion: 'apps/v1',
  kind: 'Deployment',
  metadata: {
    name: name,
    namespace: 'default',
  },
  spec: {
    replicas: 1,
    selector: {
      matchLabels: {
        app: name,
      },
    },
    template: {
      metadata: {
        labels: {
          app: name,
        },
      },
      spec: {
        containers: [{
          name: name,
          image: image,
        }],
      },
    },
  },
};

// Single inline environment
local env = {
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'inline-test',
    labels: {
      type: 'inline',
      cluster: 'test-cluster',
      fluxExport: 'false',
      inline: 'true',
    },
  },
  spec: {
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'default',
    exportJsonnetImplementation: 'binary:/usr/local/bin/jrsonnet',
  },
  data: {
    'app-config': makeConfigMap('app-config', {
      'config.yaml': std.manifestYamlDoc({
        server: {
          port: 8080,
          host: '0.0.0.0',
          test: '6e50317',
        },
        logging: {
          level: 'info',
          format: 'json',
        },
      }),
      'dashboard-to-string.json': std.toString(import 'dashboard-promtail.json'),
      'settings.json': std.manifestJson({
        debug: false,
        maxConnections: 100,
      }),
      test: '6e50317',
    }),
    'app-deployment': makeDeployment('app', 'nginx:1.25'),
  },
} + withFluxIgnore('platform-federal', 'Flux not running in federal clusters');

{
  nested: {
    nestedAgain: env,
  },
}
