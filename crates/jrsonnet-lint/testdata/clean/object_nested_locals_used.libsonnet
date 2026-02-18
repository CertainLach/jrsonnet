local vendorRules = import 'vendor.libsonnet';

{
  local metricsToEnable = [
    'alertmanager_.*',
    'container_.*',
    'http_.*',
    'node_.*',
    'up',
  ],

  local clustersToEnable = [
    'public-services-dev',
  ],

  local prologue = [
    {
      selector: '{__name__=~"(%s)", cluster=~"(%s)"}' % [std.join('|', metricsToEnable), std.join('|', clustersToEnable)],
      replacement: 'dev',
      target_label: 'env',
    },
    {
      selector: '{__name__=~"(%s)", k8s_cluster_name=~"(%s)"}' % [std.join('|', metricsToEnable), std.join('|', clustersToEnable)],
      replacement: 'dev',
      target_label: 'env',
    },
  ],

  local gated_rules = [
    {
      ranked_choice: ['cluster', 'k8s_cluster_name'],
      target_label: 'site',
    },
    {
      selector: '{site=""}',
      ranked_choice: ['client_k8s_cluster_name', 'server_k8s_cluster_name'],
      target_label: 'site',
    },
  ],

  local epilogue = [],

  local vendorNames = [
    'alertmanager',
    'prometheus',
    'redis_exporter',
  ],

  relabel_configs: {
    rules: prologue,
    gated_rules: gated_rules + vendorRules.filter(vendorNames),
    epilogue: epilogue,
  },
}
