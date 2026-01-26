// Nested std.mergePatch pattern from real code:
// 1. loki_overrides.loki.querier = std.mergePatch(super.querier + {...}, {...})
// 2. dev_overrides.loki = std.mergePatch(super.loki + {...}, {...})
// where super.loki in #2 contains the result of #1

local thor = {
  _config+:: {
    enabled: true,
    loki+:: if $._config.enabled then {
      querier+: {
        storage_start: error 'must be set',
      },
    } else {},
  },
};

// loki_overrides has std.mergePatch on super.querier
local loki_overrides = {
  _config+:: {
    loki+: {
      querier: std.mergePatch(super.querier + {
        multi_tenant: true,
      }, {
        // Does NOT null storage_start!
        other: null,
      }),
    },
  },
};

local logs_explore = { deployment: { name: 'logs' } };

// dev_overrides has std.mergePatch on super.loki
local dev_overrides = logs_explore {
  _config+:: {
    loki:: std.mergePatch(super.loki + {
      added: 'by_dev',
    }, {
      enterprise: null,
    }),
  },
};

// Nullifier nulls the error AFTER dev_overrides
local nullifier = {
  _config+:: {
    loki+:: {
      querier+: {
        storage_start:: null,
      },
    },
  },
};

// Order: thor + loki_overrides + dev_overrides + nullifier
local combined = thor + loki_overrides + dev_overrides + nullifier;

{
  apiVersion: 'v1',
  kind: 'ConfigMap',
  metadata: {
    name: 'test',
    namespace: 'default',
  },
  data: {
    'config.yaml': std.manifestYamlDoc(combined._config.loki),
  },
}
