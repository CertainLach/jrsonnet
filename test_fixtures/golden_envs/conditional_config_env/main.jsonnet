// Test cases to reproduce the condition where tk generates content but rtk doesn't
// These are more aggressive patterns based on what we see in the Loki -gf environments

// ============================================================================
// Pattern 1: Object + operator with hidden fields on right side
// This is a common pattern where a base config is extended with hidden configs
// ============================================================================
local baseConfig = {
  visible: 'base-visible',
};

local hiddenOverlay = {
  hidden:: {
    deep: {
      value: 'hidden-deep-value',
    },
  },
  // Expose the hidden field
  exposed: self.hidden,
};

// This merging pattern might behave differently
local merged1 = baseConfig + hiddenOverlay;

// ============================================================================
// Pattern 2: Accessing hidden field through $ (top-level) reference
// ============================================================================
local topLevelAccess = {
  _private:: {
    configData: {
      setting1: 'value1',
      setting2: 'value2',
      nested: {
        deep: 'deep-value',
      },
    },
  },

  // Access via $ which should reference the top of this object
  publicConfig: $._private.configData,

  // Nested access via $
  deepValue: $._private.configData.nested.deep,
};

// ============================================================================
// Pattern 3: std.mergePatch with objects containing hidden fields
// ============================================================================
local target = {
  visible: 'target',
  nested: {
    a: 1,
  },
};

local patch = {
  nested: {
    b: 2,
  },
  hidden:: 'hidden-patch',
};

local mergePatched = std.mergePatch(target, patch);

// ============================================================================
// Pattern 4: Object comprehension building from hidden source
// ============================================================================
local sourceData = {
  items:: [
    { name: 'item1', value: 'v1' },
    { name: 'item2', value: 'v2' },
    { name: 'item3', value: 'v3' },
  ],
};

local comprehensionResult = {
  [item.name]: item.value
  for item in sourceData.items
};

// ============================================================================
// Pattern 5: Conditional with std.objectHasAll (checks hidden fields)
// ============================================================================
local objWithHidden = {
  visible: 'yes',
  hidden:: 'also-yes',
};

local hasAllCheck = {
  hasVisible: std.objectHas(objWithHidden, 'visible'),
  hasHidden: std.objectHas(objWithHidden, 'hidden'),  // false for objectHas
  hasHiddenAll: std.objectHasAll(objWithHidden, 'hidden'),  // true for objectHasAll

  // Conditional based on hidden field existence
  conditionalValue: if std.objectHasAll(objWithHidden, 'hidden') then objWithHidden.hidden else 'fallback',
};

// ============================================================================
// Pattern 6: Recursive object with self-reference to hidden field
// ============================================================================
local recursive = {
  _base:: {
    name: 'test-component',
    replicas: 3,
  },

  component: {
    name: $._base.name,
    config: {
      replicas: $._base.replicas,
      labelSelector: {
        matchLabels: {
          name: $._base.name,
        },
      },
    },
  },
};

// ============================================================================
// Pattern 7: Late binding with super in extended object
// ============================================================================
local parent = {
  _config:: {
    name: 'parent-name',
  },

  getName():: self._config.name,

  result: self.getName(),
};

local child = parent {
  _config+:: {
    extra: 'child-extra',
  },

  // Override to also include parent's name
  result: super.result + '-extended',
};

// ============================================================================
// Pattern 8: Manifestation of object with hidden fields at different levels
// ============================================================================
local deepHidden = {
  level1: {
    level2:: {
      level3: {
        deepValue: 'found-it',
      },
    },
    // Expose level2
    exposed: self.level2,
  },
};

// ============================================================================
// Pattern 9: std.native call that might behave differently
// ============================================================================
local nativeManifest = {
  jsonData: { key: 'value', nested: { a: 1 } },
  yamlString: std.native('manifestYamlFromJson')(std.manifestJson(self.jsonData)),
};

// ============================================================================
// Pattern 10: Array of objects with hidden fields
// ============================================================================
local arrayOfHidden = [
  { visible: 'v1', hidden:: 'h1' },
  { visible: 'v2', hidden:: 'h2' },
];

// Try to access hidden fields from array elements
local extractedHidden = [item.hidden for item in arrayOfHidden];

// ============================================================================
// Pattern 11: Simulate the GF config pattern more closely
// This creates a config where index-gateway might be null/missing
// ============================================================================
local gfSimulation = {
  // Simulate a component that may or may not exist
  _components:: {
    indexGateway: {
      enabled: true,
      name: 'index-gateway',
      config: {
        replicas: 3,
        resources: {
          requests: { memory: '1Gi' },
        },
      },
    },
  },

  // Get component, returns null if not enabled
  getComponent(name)::
    local comp = $._components[name];
    if std.objectHas($._components, name) && comp.enabled then comp else null,

  // Build resources for a component
  buildConfig(comp)::
    if comp != null then {
      name: comp.name,
      replicas: comp.config.replicas,
    } else {},

  // The actual config - this is where empty {} might come from
  indexGatewayConfig: $.buildConfig($.getComponent('indexGateway')),

  // Service selector - this is where empty matchLabels might come from
  serviceSelector:
    local comp = $.getComponent('indexGateway');
    if comp != null then { name: comp.name } else {},
};

// ============================================================================
// Generate test ConfigMaps
// ============================================================================
{
  // Test 1: Merged with hidden overlay
  'configmap-merged-hidden': {
    assert self.apiVersion == 'v1' : 'must be core v1 API',
    assert self.kind == 'ConfigMap' : 'must be ConfigMap kind',
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      assert std.endsWith(self.name, '-test') : 'test configmaps should end with -test',
      name: 'merged-hidden-test',
      namespace: 'default',
    },
    data: {
      assert std.objectHas(self, 'merged.yaml') : 'must have merged.yaml',
      'merged.yaml': std.manifestYamlDoc(merged1),
      'exposed.yaml': std.manifestYamlDoc(merged1.exposed),
    },
  },

  // Test 2: Top-level $ access to hidden fields
  'configmap-dollar-access': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'dollar-access-test',
      namespace: 'default',
    },
    data: {
      'public.yaml': std.manifestYamlDoc(topLevelAccess.publicConfig),
      'deep.txt': topLevelAccess.deepValue,
    },
  },

  // Test 3: Comprehension from hidden source
  'configmap-comprehension': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'comprehension-test',
      namespace: 'default',
    },
    data: {
      'result.yaml': std.manifestYamlDoc(comprehensionResult),
    },
  },

  // Test 4: objectHasAll checks
  'configmap-has-all': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'has-all-test',
      namespace: 'default',
    },
    data: {
      'checks.yaml': std.manifestYamlDoc(hasAllCheck),
    },
  },

  // Test 5: Recursive self-reference
  'configmap-recursive': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'recursive-test',
      namespace: 'default',
    },
    data: {
      'component.yaml': std.manifestYamlDoc(recursive.component),
    },
  },

  // Test 6: Parent/child with super
  'configmap-super-binding': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'super-binding-test',
      namespace: 'default',
    },
    data: {
      'parent.txt': parent.result,
      'child.txt': child.result,
    },
  },

  // Test 7: Deep hidden exposure
  'configmap-deep-hidden': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'deep-hidden-test',
      namespace: 'default',
    },
    data: {
      'deep.yaml': std.manifestYamlDoc(deepHidden.level1.exposed),
    },
  },

  // Test 8: Native manifestYamlFromJson
  'configmap-native': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'native-test',
      namespace: 'default',
    },
    data: {
      'native.yaml': nativeManifest.yamlString,
    },
  },

  // Test 9: Array hidden extraction
  'configmap-array-hidden': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'array-hidden-test',
      namespace: 'default',
    },
    data: {
      'extracted.yaml': std.manifestYamlDoc(extractedHidden),
    },
  },

  // Test 10: GF simulation - the main suspect
  'configmap-gf-simulation': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'gf-simulation-test',
      namespace: 'default',
    },
    data: {
      'index-gateway.yaml': std.manifestYamlDoc(gfSimulation.indexGatewayConfig),
      'selector.yaml': std.manifestYamlDoc(gfSimulation.serviceSelector),
    },
  },

  // Test 11: PDB with selector from GF simulation
  'pdb-gf-simulation': {
    apiVersion: 'policy/v1',
    kind: 'PodDisruptionBudget',
    metadata: {
      name: 'index-gateway-pdb',
      namespace: 'default',
    },
    spec: {
      maxUnavailable: 1,
      selector: {
        matchLabels: gfSimulation.serviceSelector,
      },
    },
  },
}
