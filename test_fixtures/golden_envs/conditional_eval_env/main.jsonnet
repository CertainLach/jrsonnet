// Test cases for conditional evaluation and null handling differences between tk and rtk
// Issues reproduced from tk-compare comparison:
// 1. Empty metadata.name causing "--no-value-" filenames (ScaledObject, Service)
// 2. Empty matchLabels in PodDisruptionBudget when selector.matchLabels should have values
// 3. Service missing selector/ports when they should be present

// Simulate a component that may or may not be enabled
local componentConfig = {
  indexGateway: {
    enabled: true,  // When true, index-gateway resources should be created
    name: 'index-gateway',
    replicas: 3,
  },
  queryEngine: {
    enabled: true,
    name: 'query-engine-worker',
  },
};

// Helper to get component name, returns null if disabled
// This pattern is common in Grafana's jsonnet libraries
local getComponentName(config) =
  if config.enabled then config.name else null;

// Helper to make selector labels - returns empty object if name is null
local makeSelector(name) =
  if name != null then { name: name } else {};

// Helper that might return null for optional fields
local optionalField(condition, value) =
  if condition then value else null;

// Test: Resource with potentially null name (causes --no-value- filename)
// This simulates the ScaledObject--no-value-.yaml issue
local scaledObject(config, suffix='') = {
  apiVersion: 'keda.sh/v1alpha1',
  kind: 'ScaledObject',
  metadata: {
    // If config.enabled is false or name resolution fails, this could be null
    // Add suffix to avoid filename conflicts with Service resources
    name: getComponentName(config) + suffix,
    namespace: 'default',
  },
  spec: {
    scaleTargetRef: {
      name: getComponentName(config),
    },
    minReplicaCount: 1,
    maxReplicaCount: 10,
    triggers: [{
      type: 'prometheus',
      metadata: {
        query: 'sum(rate(requests_total[5m]))',
      },
    }],
  },
};

// Test: Service with potentially empty selector
// This simulates the Service-index-gateway-headless.yaml issue
local headlessService(config) = {
  apiVersion: 'v1',
  kind: 'Service',
  metadata: {
    name: getComponentName(config) + '-headless',
    namespace: 'default',
  },
  spec: {
    clusterIP: 'None',
    ports: [
      {
        name: getComponentName(config) + '-http-metrics',
        port: 3100,
        targetPort: 3100,
      },
      {
        name: getComponentName(config) + '-grpc',
        port: 9095,
        targetPort: 9095,
      },
    ],
    selector: {
      name: getComponentName(config),
      'rollout-group': getComponentName(config),
    },
  },
};

// Test: PodDisruptionBudget with potentially empty matchLabels
// This simulates the PodDisruptionBudget-index-gateway-pdb.yaml issue
local pdb(config) = {
  apiVersion: 'policy/v1',
  kind: 'PodDisruptionBudget',
  metadata: {
    name: getComponentName(config) + '-pdb',
    namespace: 'default',
  },
  spec: {
    maxUnavailable: 1,
    selector: {
      // This could be empty if makeSelector returns {}
      matchLabels: makeSelector(getComponentName(config)),
    },
  },
};

// Test: Object with deeply nested optional fields
local configMap(config) = {
  apiVersion: 'v1',
  kind: 'ConfigMap',
  metadata: {
    name: 'test-config',
    namespace: 'default',
    // Test annotations with optional/null values
    annotations: {
      'config.kubernetes.io/local-config': 'true',
      // This might evaluate differently between tk and rtk
      'optional-annotation': optionalField(config.indexGateway.enabled, 'enabled'),
    },
  },
  data: {
    'config.yaml': std.manifestYamlDoc({
      component: getComponentName(config.indexGateway),
      replicas: config.indexGateway.replicas,
    }),
  },
};

// Test: Conditional object inclusion pattern
// This simulates how resources are conditionally included
local conditionalResources(config) = {
  // Only include if enabled - tests conditional object creation
  [if config.indexGateway.enabled then 'index-gateway-service']: {
    apiVersion: 'v1',
    kind: 'Service',
    metadata: {
      name: 'index-gateway',
      namespace: 'default',
    },
    spec: {
      ports: [{
        name: 'http',
        port: 3100,
      }],
      selector: {
        name: 'index-gateway',
      },
    },
  },
};

// Generate all resources
{
  // ScaledObject that should have name 'index-gateway-scaler'
  'scaled-index-gateway': scaledObject(componentConfig.indexGateway, '-scaler'),
  
  // ScaledObject that should have name 'query-engine-worker-scaler'
  'scaled-query-engine': scaledObject(componentConfig.queryEngine, '-scaler'),
  
  // Headless service
  'service-headless': headlessService(componentConfig.indexGateway),
  
  // PDB
  'pdb': pdb(componentConfig.indexGateway),
  
  // ConfigMap with optional annotations
  'configmap': configMap(componentConfig),
} + conditionalResources(componentConfig)
