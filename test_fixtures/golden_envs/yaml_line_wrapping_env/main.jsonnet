// Test cases for YAML line wrapping differences between tk and rtk
// Issues reproduced from tk-compare comparison:
// 1. Shell commands breaking at different positions (e.g., before '|' vs after space)
// 2. ScaledObject PromQL queries with different line breaks
// 3. Long strings in nested contexts wrapping differently

// The exact shell command from the real codebase that shows wrapping differences:
// tk wraps: 'du -sh /data/wal/ | cut -f1'
// rtk wraps: 'du -sh /data/wal/\n| cut -f1'
local rulerPreStopCommand = 'echo -n "The current WAL directory size is: "; du -sh /data/wal/ | cut -f1; echo "Deleting WALs for all tenants"; rm -rf /data/wal/*; echo -n "After the delete the current WAL directory size is: "; du -sh /data/wal/ | cut -f1';

// PromQL query that shows different line break behavior
// tk: breaks after opening paren, rtk: breaks differently
local scaledObjectPromQL = |||
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

// Another variation of the PromQL that appears in the comparison
// This tests the specific pattern: ") * 100" on new line vs same line
local partitionIngesterQuery = |||
  (
    100 - (
      min(rate(container_cpu_usage_seconds_total{cluster=~"prod-region-4", namespace=~"loki-prod-039", container="partition-ingester", pod=~"partition-ingester-a.*"}[1m]))[15m:]
    )
    and on()
    (
      up{job=~"loki-prod-039/rollout-operator"} > 0
    )
  ) * 100      
|||;

{
  // StatefulSet ruler with pre-stop hook shell command
  // This tests the shell command line wrapping at pipe characters
  // Uses minimal nesting to match real-world Loki ruler structure (8-space indent)
  'ruler-statefulset': {
    apiVersion: 'apps/v1',
    kind: 'StatefulSet',
    metadata: {
      name: 'ruler',
      namespace: 'default',
    },
    spec: {
      command: ['/bin/bash', '-c', rulerPreStopCommand],
    },
  },

  // ScaledObject with PromQL query - tests line break in query string
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
          query: scaledObjectPromQL,
          threshold: '80',
        },
      }],
    },
  },

  // Another ScaledObject with slightly different PromQL pattern
  'partition-ingester-b-scaledobject': {
    apiVersion: 'keda.sh/v1alpha1',
    kind: 'ScaledObject',
    metadata: {
      name: 'partition-ingester-b',
      namespace: 'default',
    },
    spec: {
      scaleTargetRef: {
        name: 'partition-ingester-b',
      },
      minReplicaCount: 1,
      maxReplicaCount: 10,
      triggers: [{
        type: 'prometheus',
        metadata: {
          serverAddress: 'http://qmoojsnjst:9090',
          query: partitionIngesterQuery,
          threshold: '80',
        },
      }],
    },
  },

  // Test memcached StatefulSet - from comparison showing 87 line differences
  // Tests complex nested structure with long values
  'memcached-statefulset': {
    apiVersion: 'apps/v1',
    kind: 'StatefulSet',
    metadata: {
      name: 'memcached-extstore',
      namespace: 'default',
    },
    spec: {
      serviceName: 'memcached',
      replicas: 3,
      selector: {
        matchLabels: {
          name: 'memcached-extstore',
        },
      },
      template: {
        metadata: {
          labels: {
            name: 'memcached-extstore',
          },
        },
        spec: {
          containers: [{
            name: 'memcached',
            image: 'memcached:1.6',
            args: [
              '-m', '4096',
              '-c', '16384',
              '-I', '32m',
              // Long argument that may wrap differently
              '--extended=ext_path=/extstore/extstore:64G,ext_wbuf_size=8,ext_threads=4,ext_item_size=512,ext_item_age=3600,ext_low_ttl=3600,ext_recache_rate=1000,ext_max_frag=0.9,ext_drop_unread=true,slab_automove=2,lru_crawler,lru_maintainer',
            ],
            resources: {
              limits: {
                memory: '5Gi',
              },
              requests: {
                memory: '5Gi',
                cpu: '1',
              },
            },
          }],
        },
      },
    },
  },

  // Deployment with query-tee pattern from comparison (36 line differences)
  'query-tee-deployment': {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'query-tee',
      namespace: 'default',
    },
    spec: {
      replicas: 2,
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
              '-backend.endpoints=http://hqrvz-lcmxbnao-1.cksn.smw.unjdmak.fztow:3100,lcnd://hnsmq-cmjxkhky-2.fcqu.dqq.gyllxue.hdeig:3100,ftey://yucfy-upoysyso-3.vsuu.tku.keocpou.jzisb:3100',
              '-backend.preferred=http://okfbi-pdcmpcsu-1.vbee.zsj.chdrynt.tzthy:3100',
              '-proxy.compare-responses=true',
              '-proxy.value-comparison-tolerance=0.001',
              '-proxy.passthrough-non-registered-routes=true',
              '-server.http-listen-port=8080',
              '-server.grpc-listen-port=9095',
            ],
          }],
        },
      },
    },
  },
}
