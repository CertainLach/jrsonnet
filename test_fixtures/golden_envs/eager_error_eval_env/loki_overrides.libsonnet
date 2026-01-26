// Simulates loki-overrides.libsonnet
// IMPORTANT: This adds dev_overrides BEFORE nulling out the error
// This is key because dev_overrides.super.loki sees the error still present

local base = import 'base.libsonnet';
local dev_overrides = import 'dev_overrides.libsonnet';

// First merge: base + dev_overrides
// At this point, dev_overrides.super.loki still has the error!
local with_dev = base + dev_overrides;

// Then apply the nullification
with_dev {
  _config+:: {
    loki+:: {
      querier+: std.mergePatch(super.querier, {
        dataobj_storage_start: null,
        engine: null,
      }),
    },
  },
}
