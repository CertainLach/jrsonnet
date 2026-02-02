// Test cases for multiline string wrapping differences between tk and rtk
// Issues reproduced from tk-compare comparison:
// 1. Long shell commands breaking at different positions (| cut -f1 vs \n| cut -f1)
// 2. ScaledObject PromQL queries with different line breaks
// 3. Long command-line args wrapping differently

// Ruler shell command that exhibits wrapping differences
// tk: 'echo -n "..."; du -sh /data/wal/ | cut -f1; ...'
// rtk: 'echo -n "..."; du -sh /data/wal/\n| cut -f1; ...'
local rulerPreStopCommand = 'echo -n "The current WAL directory size is: "; du -sh /data/wal/ | cut -f1; echo "Deleting WALs for all tenants"; rm -rf /data/wal/*; echo -n "After the delete the current WAL directory size is: "; du -sh /data/wal/ | cut -f1';

// PromQL query that exhibits line break differences in ScaledObject
// The query has nested parentheses and newlines that may break differently
local scaledObjectQuery = |||
  (
    100 - (
      min(
        rate(container_cpu_usage_seconds_total{cluster=~"prod-region-1", namespace=~"loki-prod", container="partition-ingester", pod=~"partition-ingester-a.*"}[1m])
      ) / on(pod)
      max(
        rate(container_cpu_usage_seconds_total{cluster=~"prod-region-1", namespace=~"loki-prod", container="partition-ingester", pod=~"partition-ingester-a.*"}[1m])
      )[15m:]
    )
    and on()
    (
      up{job=~"loki-prod/rollout-operator"} > 0
    )
  ) * 100
|||;

// Alternative formulation that's all on one line (tests inline wrapping)
local inlinePromqlQuery = '(100 - (min(rate(container_cpu_usage_seconds_total{cluster=~"prod-region-1", namespace=~"loki-prod", container="partition-ingester", pod=~"partition-ingester-a.*"}[1m])) / on(pod) max(rate(container_cpu_usage_seconds_total{cluster=~"prod-region-1", namespace=~"loki-prod", container="partition-ingester", pod=~"partition-ingester-a.*"}[1m]))[15m:])) and on() (up{job=~"loki-prod/rollout-operator"} > 0)) * 100';

{
  // StatefulSet with ruler that has pre-stop hook with shell command
  'ruler-statefulset': {
    assert self.kind == 'StatefulSet' : 'must be StatefulSet',
    apiVersion: 'apps/v1',
    kind: 'StatefulSet',
    metadata: {
      assert std.length(self.name) > 0 : 'metadata.name required',
      name: 'ruler',
      namespace: 'default',
    },
    spec: {
      assert self.serviceName == 'ruler' : 'serviceName should be ruler',
      serviceName: 'ruler',
      replicas: 1,
      selector: {
        matchLabels: {
          name: 'ruler',
        },
      },
      template: {
        metadata: {
          labels: {
            assert self.name == 'ruler' : 'pod label name must match',
            name: 'ruler',
          },
          annotations: {
            config_hash: 'abc123',
          },
        },
        spec: {
          assert std.length(self.containers) == 1 : 'should have exactly 1 container',
          containers: [{
            assert self.name == 'ruler' : 'container name must be ruler',
            name: 'ruler',
            image: 'grafana/loki:latest',
            lifecycle: {
              preStop: {
                exec: {
                  assert std.length(self.command) == 3 : 'preStop command should have 3 parts',
                  // This command triggers different line wrapping
                  command: ['/bin/bash', '-c', rulerPreStopCommand],
                },
              },
            },
          }],
        },
      },
    },
  },

  // ScaledObject with PromQL query that breaks at different positions
  'partition-ingester-scaledobject': {
    apiVersion: 'keda.sh/v1alpha1',
    kind: 'ScaledObject',
    metadata: {
      name: 'partition-ingester-a',
      namespace: 'default',
    },
    spec: {
      scaleTargetRef: {
        name: 'partition-ingester-a',
      },
      minReplicaCount: 1,
      maxReplicaCount: 10,
      triggers: [{
        type: 'prometheus',
        metadata: {
          serverAddress: 'http://qmoojsnjst:9090',
          // Multi-line query that triggers different line breaking
          query: scaledObjectQuery,
          threshold: '80',
        },
      }],
    },
  },

  // Another ScaledObject with inline query
  'query-engine-worker-scaledobject': {
    apiVersion: 'keda.sh/v1alpha1',
    kind: 'ScaledObject',
    metadata: {
      name: 'query-engine-worker',
      namespace: 'default',
    },
    spec: {
      scaleTargetRef: {
        name: 'query-engine-worker',
      },
      minReplicaCount: 1,
      maxReplicaCount: 20,
      triggers: [{
        type: 'prometheus',
        metadata: {
          serverAddress: 'http://qmoojsnjst:9090',
          query: inlinePromqlQuery,
          threshold: '50',
        },
      }],
    },
  },

  // Deployment with long args that trigger line wrapping
  'query-tee-deployment': {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'query-tee',
      namespace: 'default',
    },
    spec: {
      replicas: 1,
      selector: {
        matchLabels: {
          name: 'query-tee',
        },
      },
      template: {
        metadata: {
          labels: {
            name: 'query-tee',
          },
        },
        spec: {
          containers: [{
            name: 'query-tee',
            image: 'grafana/query-tee:latest',
            args: [
              // Long args that may wrap differently
              '-backend.endpoints=http://hjvsl-prqlnxrk-1.lwoi.jih.dxgkdxr.ywfgg:3100,xpix://syqtj-dgjhlfja-2.wrei.bbr.jweuypd.asjpp:3100',
              '-backend.preferred=http://okfbi-pdcmpcsu-1.vbee.zsj.chdrynt.tzthy:3100',
              '-proxy.compare-responses=true',
              '-proxy.value-comparison-tolerance=0.001',
            ],
          }],
        },
      },
    },
  },

  // ConfigMap with limit-updater deny lists (has different line count in comparison)
  'limit-updater-deny-lists-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'limit-updater-deny-lists',
      namespace: 'default',
    },
    data: {
      // YAML content that may have different formatting
      'deny-lists.yaml': std.manifestYamlDoc({
        deny_lists: {
          retention_period: [
            'tenant-1',
            'tenant-2',
            'tenant-3',
          ],
          ingestion_rate: [
            'high-volume-tenant',
          ],
        },
      }),
    },
  },

  // ConfigMap with autoscaler metrics (large line differences in comparison)
  'loki-autoscaler-metrics-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'loki-autoscaler-metrics',
      namespace: 'default',
    },
    data: {
      'metrics.yaml': std.manifestYamlDoc({
        metrics: [
          {
            name: 'cpu_utilization',
            query: 'sum(rate(container_cpu_usage_seconds_total{namespace="loki"}[5m])) by (pod)',
            threshold: 0.8,
          },
          {
            name: 'memory_utilization',
            query: 'sum(container_memory_working_set_bytes{namespace="loki"}) by (pod) / sum(container_spec_memory_limit_bytes{namespace="loki"}) by (pod)',
            threshold: 0.9,
          },
          {
            name: 'disk_utilization',
            query: '1 - (sum(kubelet_volume_stats_available_bytes{namespace="loki"}) by (persistentvolumeclaim) / sum(kubelet_volume_stats_capacity_bytes{namespace="loki"}) by (persistentvolumeclaim))',
            threshold: 0.85,
          },
        ],
      }),
    },
  },
}
