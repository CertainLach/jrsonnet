// Test fixture for static-exporter-style ConfigMaps with large multiline content
// This tests YAML serialization of large imported text files

local httpdConf = importstr 'httpd.conf';

{
  apiVersion: 'v1',
  kind: 'ConfigMap',
  metadata: {
    name: 'httpd-config',
    namespace: 'default',
    annotations: null,
  },
  data2: {
    HELLOAWORLD: 'hello',
    HELLO_WORLD: 'hello',
  },
  data: {
    HELLO_WORLD: 'hello',
    HELLOAWORLD: 'hello',
    'httpd.conf': httpdConf,
    'grafanacloud_usage_group/spanmetrics_service_unknown_service:java': 'hello',
  },
}
