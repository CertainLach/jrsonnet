// Test case for float rounding in std.format with %0.0f
// Issue: go-jsonnet rounds 7730941132.8 to 7730941133, jrsonnet truncates to 7730941132
// This reproduces a real-world issue from Tempo environments where GOMEMLIMIT is computed
// from memory limits using percentage calculations.

// Helper to calculate GOMEMLIMIT from memory limit and percentage
// This mirrors the real-world calculation: memoryLimitBytes * percentage / 100
local memoryLimitBytes(m) =
  if std.endsWith(m, 'Gi') then
    std.parseInt(std.split(m, 'Gi')[0]) * 1024 * 1024 * 1024
  else if std.endsWith(m, 'Mi') then
    std.parseInt(std.split(m, 'Mi')[0]) * 1024 * 1024
  else if std.endsWith(m, 'Ki') then
    std.parseInt(std.split(m, 'Ki')[0]) * 1024
  else if std.endsWith(m, 'B') then
    std.parseInt(std.split(m, 'B')[0])
  else
    error 'Unknown memory unit';

local memoryLimitPercent(m, p) =
  '%0.0f' % (memoryLimitBytes(m) * p / 100);

// The exact values from the Tempo env that exposed this issue:
// 9Gi * 80% = 9 * 1024 * 1024 * 1024 * 80 / 100 = 7730941132.8
// go-jsonnet: 7730941133 (rounds .8 up)
// jrsonnet:   7730941132 (truncates)
local testMemory = '9Gi';
local testPercent = 80;

// Other rounding edge cases
local roundingTests = {
  // Value ending in .5 - should round up (banker's rounding vs standard)
  halfUp: '%0.0f' % 2.5,  // go: 2 (rounds to even), or 3?
  halfDown: '%0.0f' % 1.5,  // go: 2 (rounds to even), or 2?

  // Values just below and above .5
  belowHalf: '%0.0f' % 2.4,  // should be 2
  aboveHalf: '%0.0f' % 2.6,  // should be 3

  // Large numbers with fractional parts
  largeUp: '%0.0f' % (9 * 1024 * 1024 * 1024 * 80 / 100),  // 7730941132.8 -> should be 7730941133
  largeDown: '%0.0f' % (9 * 1024 * 1024 * 1024 * 70 / 100),  // 6764573491.2 -> should be 6764573491

  // Negative numbers
  negativeUp: '%0.0f' % -2.4,  // should be -2
  negativeDown: '%0.0f' % -2.6,  // should be -3
};

{
  'rounding-test-configmap': {
    assert self.kind == 'ConfigMap' : 'must be ConfigMap',
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      assert self.namespace == 'default' : 'namespace must be default',
      name: 'rounding-test',
      namespace: 'default',
    },
    data: {
      assert std.endsWith(self.gomemlimit, 'B') : 'gomemlimit must end with B',
      // The main GOMEMLIMIT calculation that exposed the issue
      gomemlimit: memoryLimitPercent(testMemory, testPercent) + 'B',
      // Additional rounding test cases
      'rounding-tests.json': std.manifestJsonEx(roundingTests, '  '),
    },
  },
  'statefulset-live-store': {
    assert self.kind == 'StatefulSet' : 'must be StatefulSet',
    apiVersion: 'apps/v1',
    kind: 'StatefulSet',
    metadata: {
      name: 'live-store',
      namespace: 'default',
    },
    spec: {
      template: {
        spec: {
          containers: [{
            assert std.length(self.env) == 2 : 'should have 2 env vars',
            name: 'live-store',
            image: 'cloud-traces:latest',
            env: [
              { assert self.name == 'GOGC' : 'first env should be GOGC', name: 'GOGC', value: '100' },
              { assert self.name == 'GOMEMLIMIT' : 'second env should be GOMEMLIMIT', name: 'GOMEMLIMIT', value: memoryLimitPercent(testMemory, testPercent) + 'B' },
            ],
            resources: {
              assert std.objectHas(self.limits, 'memory') : 'must have memory limit',
              limits: {
                memory: testMemory,
              },
            },
          }],
        },
      },
    },
  },
}
