// Test case for nested YAML block scalar indentation
// This reproduces the issue where rtk adds extra indentation to block scalar content
// compared to tanka (Go implementation), causing different raw string values
// even though double-parsing produces equivalent results.

// Test case: YAML list content should use block scalar style, not double-quoted
// When the string starts with "- " (YAML list indicator) and contains newlines,
// Go yaml.v3 uses block literal style (|) but serde-saphyr was incorrectly
// using double-quoted because is_plain_value_safe() returned false for "- ".
// Block scalars can contain ANY content - they're literal, not parsed as YAML.
local yamlListContent = |||
  - item1
  - item2
  - item3
|||;

// Test case: Same content but WITHOUT trailing newline (like std.manifestYamlDoc produces)
// This is the actual failing case - content starting with "- " and no trailing newline
local yamlListNoTrailing = '- item1\n- item2\n- item3';

// Test case: Content with literal \n in the text (like escape sequences)
// This is the actual trigger - content that has \n as text, not as newlines
local contentWithBackslashN = '- line1\\n- line2\\n- line3';

// Test case: Content with literal \n mixed with actual newlines
local mixedContent = '- item with \\n escape\n- another item\n- third with \\n too';

// Test case: Content with empty lines (double newlines)
local contentWithEmptyLines = '- item1\n\n- item2\n\n- item3';

// Test case: Content with multiple empty lines
local contentWithMultipleEmptyLines = '- item1\n\n\n- item2';

// Test case: Empty line at start
local emptyLineAtStart = '\n- item1\n- item2';

// Test case: Content with empty line in middle (block text style)
local blockWithEmptyLine = |||
  line1

  line3
|||;

// Test case: YAML list with empty lines between items
local yamlListWithEmptyLines = |||
  - item1

  - item2

  - item3
|||;

// Test case: Production-like content - YAML list without quotes around items
// This is how production k8s-monitoring-static-usages generates content
// Adding MANY items to match production size
local productionLikeContent = std.join('\n', [
  '- asserts:node:count',
  '- sum by (cluster) (asserts:node_memory_MemAvailable_bytes:sum{})',
  '- sum(kube_namespace_status_phase{asserts_env!=""} == 1) by (cluster, namespace, asserts_env, asserts_site)',
  '- sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_requests:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"})) / sum(max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  '- sum(node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"}) / sum(max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  '- sum(node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"}) / sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_requests:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"}))',
  '- sum(max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  '- sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_limits:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"}))',
  '- sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_requests:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"}))',
  '- sum(node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"})',
  '- group by (cluster, node) (ALERTS{node!="", cluster=~"${cluster:pipe}", alertstate=~"firing", alertname=~"Kube.*|CPUThrottlingHigh"})',
  '- max by (cluster, node, os_image) (kube_node_info{cluster=~"$cluster", node=~"${node:pipe}"})',
  '- count by (cluster, node) (ALERTS{node!="", cluster=~"${cluster:pipe}", alertstate=~"firing", alertname=~"Kube.*|CPUThrottlingHigh"})',
  '- last_over_time((max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))[42m:])',
  '- quantile_over_time(0.95, sum by (cluster, node) (node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"})[42m:42s])',
  '- sum by (cluster) (node_filesystem_size_bytes{mountpoint=~"/"})',
  '- sum by (cluster) (max by (cluster, node, resource) (kube_node_status_capacity{resource=~"memory"}))',
  '- 1985 + 1986',
  '- (1986 + 1986) /2',
  '- (1986 + 1986) / 2',
  '- 1985 / 1986',
  '- 1 - (1986 / 1986)',
  '- 1985 + (1986 * 1986)',
]);

// Another test: content starting with YAML mapping indicator
local yamlMappingContent = |||
  key: value
  other: data
|||;

// Simulated PromQL queries - adding many to reproduce production-like content
local queriesData = [
  // From production k8s-monitoring-static-usages
  'asserts:node:count',
  'sum by (cluster) (asserts:node_memory_MemAvailable_bytes:sum{})',
  'sum(kube_namespace_status_phase{asserts_env!=""} == 1) by (cluster, namespace, asserts_env, asserts_site)',
  'sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_requests:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"})) / sum(max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  'sum(node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"}) / sum(max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  'sum(node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"}) / sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_requests:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"}))',
  'sum(max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  'sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_limits:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"}))',
  'sum(max by (cluster, namespace) (namespace_cpu:kube_pod_container_resource_requests:sum{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}"}))',
  'sum(node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"})',
  'sum by (cluster, node) (node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"}) / on (cluster) group_left() sum by (cluster) (max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  'group by (cluster, node) (ALERTS{node!="", cluster=~"${cluster:pipe}", alertstate=~"firing", alertname=~"Kube.*|CPUThrottlingHigh"})',
  'sum by (cluster, node) (node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"}) / on (cluster, node) group_left() sum by (cluster, node) (max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))',
  'max by (cluster, node, os_image) (kube_node_info{cluster=~"$cluster", node=~"${node:pipe}"})',
  'count by (cluster, node) (ALERTS{node!="", cluster=~"${cluster:pipe}", alertstate=~"firing", alertname=~"Kube.*|CPUThrottlingHigh"})',
  'last_over_time((max by (cluster, node) (kube_node_status_capacity{cluster=~"${cluster:pipe}", node=~"${node:pipe}"}))[42m:])',
  'quantile_over_time(0.95, sum by (cluster, node) (node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{cluster=~"${cluster:pipe}", namespace=~"${namespace:pipe}", node=~"${node:pipe}"})[42m:42s])',
  'sum by (cluster) (node_filesystem_size_bytes{mountpoint=~"/"})',
  'sum by (cluster) (max by (cluster, node, resource) (kube_node_status_capacity{resource=~"memory"}))',
  '1985 + 1986',
  '(1986 + 1986) /2',
  '(1986 + 1986) / 2',
  '1985 / 1986',
  '1 - (1986 / 1986)',
  '1985 + (1986 * 1986)',
  // Multi-line query using block scalar
  |||
    sum(
          floor(
            max by (cluster, node) (
              kube_node_status_capacity{resource="cpu", cluster=~"(${cluster:pipe})"}
            )
            - on (cluster, node) group_left(pod) (
              sum by (cluster, node) (
                cluster:namespace:pod_cpu:active:kube_pod_container_resource_requests{cluster=~"(${cluster:pipe})"}
              )
            )
          )
          * on (cluster, node) group_left()
          max by (cluster, node) (node_cpu_hourly_cost{cluster=~"(${cluster:pipe})"})
        ) * 24 * 30
  |||,
  'sum(max by (cluster, node) (node_total_hourly_cost{cluster=~"(${cluster:pipe})"}))',
];

{
  // ConfigMap with nested YAML containing block scalars
  'queries-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'k8s-monitoring-static-usages',
      namespace: 'default',
    },
    data: {
      'customdata.yaml': [
        {
          ['float-%s' % i]: 'test=%s' % (0.1 + 0.1 * i),
        }
        for i in std.range(0, 1000)
      ],
      // This YAML string contains block scalars (|-) for the multi-line queries
      // When the outer ConfigMap is serialized to YAML, the inner block scalar
      // indentation affects the raw string value stored in queries.yaml
      'queries.yaml': std.manifestYamlDoc(queriesData),
      // Test: YAML list content starting with "- " should use block scalar style
      'list.yaml': yamlListContent,
      // Test: YAML mapping content starting with "key:" should use block scalar style
      'mapping.yaml': yamlMappingContent,
      // Test: Same but without trailing newline - this is the failing case
      'list-no-trailing.yaml': yamlListNoTrailing,
      // Test: Production-like content - YAML list without quotes (like production generates)
      'production-like.yaml': productionLikeContent,
      // Test: Content with backslash-n (escape sequences)
      'backslash-n.yaml': contentWithBackslashN,
      // Test: Mixed backslash-n and real newlines
      'mixed.yaml': mixedContent,
      // Test: Content with empty lines
      'empty-lines.yaml': contentWithEmptyLines,
      // Test: Content with multiple empty lines
      'multi-empty-lines.yaml': contentWithMultipleEmptyLines,
      // Test: Empty line at start
      'empty-line-start.yaml': emptyLineAtStart,
      // Test: Block with empty line in middle
      'block-empty-line.yaml': blockWithEmptyLine,
      // Test: YAML list with empty lines
      'yaml-list-empty-lines.yaml': yamlListWithEmptyLines,
      roundtrip: std.manifestYamlDoc(std.parseYaml(std.parseYaml(importstr 'ConfigMap-k8s-monitoring-static-usages.yaml').data['queries.yaml'])),
      manifest_yaml_from_json: std.native('manifestYamlFromJson')(std.manifestJson(std.parseYaml(std.parseYaml(importstr 'ConfigMap-parallel-read-path-overrides.yaml').data['overrides.yaml']))),
      manifest_yaml_from_json_k8s_static_usages: std.native('manifestYamlFromJson')(std.manifestJson(std.parseYaml(std.parseYaml(importstr 'ConfigMap-k8s-monitoring-static-usages.yaml').data['queries.yaml']))),
      manifest_yaml_automation_prometheus_mimir_mixin: std.native('manifestYamlFromJson')(std.toString({
        objs: [{
          array: [
            {
              hello: |||
                my mu
                lti
                line
                string
              |||,
              otherattr: 'infinity',
              record: 'namespace_user:cortex_ingester_owned_target_info_series:sum_filtered_max_over_time_1d',
            },
          ],
        }],
      })),
    },
  },
  scaledobject: std.parseYaml(importstr 'ScaledObject-ingester-zone-a.yaml'),
  k8s_monitoring_static_usages: std.parseYaml(importstr 'ConfigMap-k8s-monitoring-static-usages.yaml'),
  tempo_create_deployment_thursday: std.parseYaml(importstr 'CronWorkflow-tempo-create-deployment-thursday-prod.yaml'),
  workflow_patch_test_patch: std.parseYaml(importstr 'Workflow-patch-test-patch.yaml'),
  ge_grafana_plugins_config: std.parseYaml(importstr 'ConfigMap-ge-grafana-plugins-config.yaml'),
  parallel_read_path_overrides: std.parseYaml(importstr 'ConfigMap-parallel-read-path-overrides.yaml'),
  overrides_configmap: std.parseYaml(importstr 'ConfigMap-overrides.yaml'),
  mimir_emojis: std.parseYaml(importstr 'ConfigMap-emojis.yaml'),
}
