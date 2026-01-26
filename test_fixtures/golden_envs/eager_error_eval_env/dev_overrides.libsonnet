// Simulates dev-overrides.libsonnet
// This is merged AFTER loki_overrides.libsonnet does its mergePatch
local logs_explore = import 'logs_explore.libsonnet';

logs_explore {
  _config+:: {
    // Uses :: (final) and std.mergePatch on super.loki
    // At this point, super.loki should have the error nulled out already
    loki:: std.mergePatch(super.loki + {
      querier+: {
        per_request_limits: true,
      },
    }, {
      enterprise_limits: null,
    }),
  },
}
