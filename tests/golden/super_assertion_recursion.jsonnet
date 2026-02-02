// Minimal reproduction of infinite recursion bug with super + assertions
// This pattern is from loki's compactor-worker.libsonnet + arm.libsonnet
//
// The issue: when using super in field definition AND having an assertion
// that accesses that same field via $, jrsonnet detects infinite recursion
// but go-jsonnet handles it correctly.

local base = {
  deployment: { spec: { replicas: 1 } },
};

local autoscaling = {
  _config+:: {
    autoscaling: {
      enabled: false,
      // This assertion accesses $.deployment, which triggers the recursion
      assert (
        if self.enabled
        then std.get($.deployment.spec, 'replicas') == null
        else $.deployment.spec.replicas == 1
      ) : 'replicas check failed',
    },
  },

  // This field uses super AND the condition depends on _config which has the assertion
  deployment: std.mergePatch(
    super.deployment,
    if $._config.autoscaling.enabled then { spec+: { replicas: null } } else {},
  ),
};

local arm = {
  // This function mimics overrideSuperIfExists from arm.libsonnet
  local overrideSuperIfExists(name, override) =
    if !( name in super) || super[name] == null || super[name] == {}
    then null
    else super[name] + override,

  deployment: overrideSuperIfExists('deployment', {}),
};

// The combination triggers the bug
base + autoscaling + arm
