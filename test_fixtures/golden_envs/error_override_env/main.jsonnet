// Tests the exact pattern where dev-overrides uses loki:: (final field)
// which prevents later overrides from working

local base = {
  _config+:: {
    thor_engine: {
      enabled: true,
    },
    
    loki+:: if $._config.thor_engine.enabled then {
      querier+: {
        // This error should be nulled out
        dataobj_storage_start: error 'must be set',
        engine_v2+: {
          dataobj_storage_start: $._config.loki.querier.dataobj_storage_start,
        },
      },
      query_engine: {
        storage_start_date: error 'storage_start_date is not defined',
      },
    } else {},
  },
};

local release_configs = {
  _config+:: {
    loki+:: if $._config.thor_engine.enabled then {
      querier: std.mergePatch(super.querier, {
        dataobj_storage_start: null,
        engine_v2: null,
      }),
    } else {},
  },
};

// Simulates dev-overrides.libsonnet which uses loki:: (FINAL field with double colon)
local dev_overrides = {
  _config+:: {
    // Using loki:: makes this a FINAL field - cannot be overridden later!
    loki:: std.mergePatch(super.loki + {
      extra_field: 'added',
    }, {
      // Null out enterprise limits
      enterprise_limits: null,
    }),
  },
};

// Simulates temp.libsonnet trying to override after dev_overrides
local temp_overrides = {
  _config+:: {
    loki+:: {
      query_engine+: {
        storage_start_date: '2025-01-08',
      },
    },
  },
};

local combined = base + release_configs + dev_overrides + temp_overrides;

{
  apiVersion: 'v1',
  kind: 'ConfigMap',
  metadata: {
    name: 'test-config',
    namespace: 'default',
  },
  data: {
    'config.yaml': std.manifestYamlDoc(combined._config.loki),
  },
}
