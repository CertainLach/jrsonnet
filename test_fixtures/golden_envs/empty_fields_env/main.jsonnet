// Test cases for empty/null field handling differences between tk and rtk
// Issues reproduced from tk-compare comparison:
// 1. Empty matchLabels in PodDisruptionBudget (tk: {name: x}, rtk: {})
// 2. Empty annotations in Ingress resources
// 3. Potential null/empty resource name issues

// Simulate the pattern where index-gateway config is conditionally included
local indexGatewayEnabled = true;
local indexGatewayName = if indexGatewayEnabled then 'index-gateway' else null;

// Simulate a mixin pattern where labels might be conditionally set
local makeLabels(name) =
  if name != null then { name: name } else {};

// Test case: PodDisruptionBudget with potentially empty matchLabels
// In the real codebase, this happens when a resource is disabled but
// the PDB template is still generated
{
  // PodDisruptionBudget that should have non-empty matchLabels
  'pdb-with-labels': {
    assert self.apiVersion == 'policy/v1' : 'must use policy/v1',
    assert self.kind == 'PodDisruptionBudget' : 'must be PDB',
    apiVersion: 'policy/v1',
    kind: 'PodDisruptionBudget',
    metadata: {
      assert std.endsWith(self.name, '-pdb') : 'PDB names should end with -pdb',
      name: 'index-gateway-pdb',
      namespace: 'default',
    },
    spec: {
      assert self.maxUnavailable >= 1 : 'maxUnavailable must be at least 1',
      maxUnavailable: 1,
      selector: {
        matchLabels: makeLabels(indexGatewayName),
      },
    },
  },

  // Ingress with annotations that may be empty in some cases
  // tk: annotations with values, rtk: annotations: {}
  'ingress-with-annotations': {
    apiVersion: 'networking.k8s.io/v1',
    kind: 'Ingress',
    metadata: {
      name: 'cortex-gw',
      namespace: 'default',
      // Test with a conditional annotation block
      annotations: {
        'kubernetes.io/ingress.class': 'nginx',
        'nginx.ingress.kubernetes.io/ssl-redirect': 'true',
      },
    },
    spec: {
      rules: [{
        host: 'cortex.example.com',
        http: {
          paths: [{
            path: '/',
            pathType: 'Prefix',
            backend: {
              service: {
                name: 'cortex-gw',
                port: { number: 80 },
              },
            },
          }],
        },
      }],
    },
  },

  // Test empty annotations object vs omitted annotations
  'ingress-empty-annotations': {
    apiVersion: 'networking.k8s.io/v1',
    kind: 'Ingress',
    metadata: {
      name: 'logs-ingress',
      namespace: 'default',
      // Empty annotations object - behavior may differ
      annotations: {},
    },
    spec: {
      rules: [{
        host: 'logs.example.com',
        http: {
          paths: [{
            path: '/',
            pathType: 'Prefix',
            backend: {
              service: {
                name: 'logs',
                port: { number: 80 },
              },
            },
          }],
        },
      }],
    },
  },

  // Service with potentially empty selector
  'service-with-selector': {
    assert self.kind == 'Service' : 'must be Service',
    apiVersion: 'v1',
    kind: 'Service',
    metadata: {
      name: 'index-gateway',
      namespace: 'default',
    },
    spec: {
      assert std.length(self.ports) == 2 : 'should have exactly 2 ports',
      ports: [{
        assert self.port > 0 && self.port < 65536 : 'port must be valid',
        name: 'http-metrics',
        port: 3100,
        targetPort: 3100,
      }, {
        assert self.name == 'grpc' : 'second port should be grpc',
        name: 'grpc',
        port: 9095,
        targetPort: 9095,
      }],
      selector: makeLabels(indexGatewayName),
    },
  },

  // Service headless variant
  'service-headless': {
    apiVersion: 'v1',
    kind: 'Service',
    metadata: {
      name: 'index-gateway-headless',
      namespace: 'default',
    },
    spec: {
      clusterIP: 'None',
      ports: [{
        name: 'http-metrics',
        port: 3100,
        targetPort: 3100,
      }, {
        name: 'grpc',
        port: 9095,
        targetPort: 9095,
      }],
      selector: {
        name: indexGatewayName,
        'rollout-group': 'index-gateway',
      },
    },
  },

  // Deployment with empty/conditional env
  'deployment-conditional-env': {
    assert self.apiVersion == 'apps/v1' : 'must use apps/v1',
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      assert std.isString(self.name) && std.length(self.name) > 0 : 'name required',
      name: 'test-deployment',
      namespace: 'default',
    },
    spec: {
      assert self.replicas >= 0 : 'replicas must be non-negative',
      replicas: 1,
      selector: {
        matchLabels: {
          assert std.objectHas(self, 'app') : 'must have app label',
          app: 'test',
        },
      },
      template: {
        metadata: {
          labels: {
            app: 'test',
          },
          annotations: {
            assert std.length(self.config_hash) == 32 : 'md5 hash should be 32 chars',
            // Config hash that depends on underlying ConfigMap content
            config_hash: std.md5(std.manifestJson({
              setting1: 'value1',
              setting2: 'value2',
            })),
          },
        },
        spec: {
          assert std.isArray(self.containers) : 'containers must be array',
          containers: [{
            assert self.name == 'test' : 'container name must be test',
            name: 'test',
            image: 'test:latest',
            // Empty env array
            env: [],
          }],
        },
      },
    },
  },
}
