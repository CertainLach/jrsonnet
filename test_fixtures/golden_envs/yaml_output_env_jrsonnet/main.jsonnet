// Test cases for YAML serialization compatibility between tk (go-yaml) and rtk (serde-saphyr)

local manifestYaml = function(value) (
  local f = std.native('manifestYamlFromJson');
  if f != null
  then f(std.toString(value))
  else std.manifestYamlDoc(value)
);


// Grafana datasource structure - tests quote_keys=true with nested arrays
// This reproduces the pattern from grafana-o11y ConfigMap-grafana-datasources.yaml
local datasourceConfigs = {
  'critical-prometheus.yml': {
    apiVersion: 1,
    datasources: [{
      access: 'proxy',
      editable: false,
      isDefault: false,
      jsonData: {
        exemplarTraceIdDestinations: [{
          datasourceUid: 'tempo-ops-01',
          name: 'traceID',
        }],
        httpMethod: 'GET',
      },
      name: 'critical-prometheus',
      type: 'prometheus',
      uid: 'critical-prometheus',
      url: 'http://kypgheec-jzdqyrxqbv.kytetmeffwytz.fxo.bnlvxho.tjyxz./ohrhjzzp-glvsooyjym/',
      version: 1,
    }],
  },
  'grafana-billing-svc.yml': {
    apiVersion: 1,
    datasources: [{
      access: 'proxy',
      basicAuth: true,
      basicAuthUser: '1',
      editable: false,
      isDefault: false,
      jsonData: {
        httpMethod: 'POST',
        manageAlerts: false,
      },
      name: 'grafana-billing-svc',
      secureJsonData: {
        basicAuthPassword: '$BILLING_VIEW_KEY',
      },
      type: 'prometheus',
      uid: 'grafana-billing-svc',
      url: 'http://qqnfgzt-eibuplrd.wqdjcyf-ioq.enc.hola-nbmqlh-1.mclha/kdw/ukze',
      version: 1,
    }],
  },
  'loki-ops.yml': {
    apiVersion: 1,
    datasources: [{
      access: 'proxy',
      basicAuth: true,
      basicAuthUser: '29',
      editable: false,
      isDefault: false,
      jsonData: {
        derivedFields: [{
          datasourceUid: 'tempo-ops-01',
          matcherRegex: '(?:traceID|trace_id|tid)=(\\w+)',
          name: 'TraceID',
          url: '$${__value.raw}',
        }],
        httpMethod: 'GET',
      },
      name: 'loki-ops',
      secureJsonData: {
        basicAuthPassword: '$GRAFANA_LOKI_READ_KEY_LOG',
      },
      type: 'loki',
      uid: 'loki-ops',
      url: 'https://ebjp-wey-002.wpotaca-ige.rxt',
      version: 1,
    }],
  },
};

// Dashboard provisioning structure - tests quote_keys=true with nested arrays
local dashboardProvisioningConfig = {
  apiVersion: 1,
  providers: [{
    allowUiUpdates: false,
    disableDeletion: true,
    editable: true,
    folder: 'General',
    folderUid: '',
    name: 'default',
    options: {
      path: '/var/lib/grafana/dashboards',
    },
    orgId: 1,
    type: 'file',
    updateIntervalSeconds: 3,
  }, {
    allowUiUpdates: false,
    disableDeletion: true,
    editable: true,
    folder: 'Alerting',
    folderUid: 'alerting',
    name: 'alerting',
    options: {
      path: '/var/lib/grafana/dashboards/alerting',
    },
    orgId: 1,
    type: 'file',
    updateIntervalSeconds: 3,
  }],
};

// Prometheus alerting rules structure - tests deeply nested object-in-array indentation
// This structure specifically tests: array -> object -> field with array value -> object -> nested object
local alertingRulesData = {
  groups: [{
    name: 'prometheus-extra',
    rules: [{
      alert: 'PromScrapeFailed',
      annotations: {
        message2: "hello'",
        message3: 'hello,',
        message: "Prometheus failed to scrape a target's job {{ $labels.job }} / {{ $labels.instance }} %s" % 0.8,
        runbook_url: 'https://uctufr.xhf/omiuwrh/vkyybhbzkm_icgsw/kgup/muoeie/hxrr/ixsnsm/lheqpcyb.de#UfzvntcMmtoQiravxdvryiy',
      },
      expr: 'up != 1',
      'for': '15m',
      labels: {
        severity: 'warning',
        expr: 'max by (namespace, provider, collector) (cloudcost_exporter_collector_last_scrape_duration_seconds) > 50',
        bytes_threshold: 7500000,
        message: "There are running tests that don't have any metric rows inserted for 10 minutes",
        message2: 'No new in-progress runners for over 20m while jobs remain queued. This might indicate rate limiting, scheduling issues, etc.',
        expr2: 'vector(1)',
        message3: 'ChatOps proxy experiencing 502,503,504 errors',
        message4: |||
          This is a multiline string.
          This is a second line. It has an intentional trailing space. tk mangles it.

        |||,
        // Test string with escaped double quotes inside - should be quoted
        summary: 'PersistentVolume has been in "released" state for more than a week.',
        // Test multiline string with multiple trailing newlines (|2+ indicator)
        rules: |||
          alert: TestAlert
          expr: up == 1

        |||,
      },
    }, {
      alert: 'PromScrapeFlapping',
      annotations: {
        message: 'Prometheus target flapping {{ $labels.job }} / {{ $labels.instance }}',
        description: '{{ printf "%.1f" $value }}% minimum errors while sending alerts from any Prometheus server in HA group {{$labels.job}} in {{$labels.cluster}} to any Alertmanager.',
        description2: 'The 95th percentile of LZ allocation step duration has been >= 290ms for the last 20 minutes.',
      },
      expr: 'avg_over_time(up[5m]) < 1',
      record: 'cluster_job:adaptive_logs_gateway_request_duration_seconds:50quantile',
      'for': '15m',
      labels: {
        severity: 'warning',
      },
    }],
  }],
};

// Nested YAML data for literal block test - tests indentation of nested mappings
local nestedYamlData = {
  enabled: true,
  rules: [
    {
      labels: {
        clientId: '$3',
        partition: '$5',
        topic: '$4',
      },
      nested_again: [
        {
          a: 'c',
        },
        {
          b: 'd',
        },
      ],
      name: 'test_rule_1',
      // String with angle brackets and colon - tk quotes, rtk doesn't
      pattern: 'server<type=metrics, listener=(.+), processor=(.+)><>(.+):',
    },
  ],
};

local configData = {
  database: {
    host: 'localhost',
    port: 5432,
    name: 'myapp',
  },
  features: {
    enableCache: true,
    float: 8.1,
    // Test large float formatting: tk uses scientific notation for large numbers,
    // rtk may not. 3333333.333333333 vs 3.333333333333333e+06
    max_series: 10000000 / 3,
  },
  servers: [
    {
      name: 'server1',
      ip: '10.0.0.1',
      dependencies: [
        {
          name: 'dependency1',
          version: '1.0.0',
          dependencies: [
            {
              name: 'dependency2',
              version: '2.0.0',
              description: |||
                This is a dependency description.
                This is a second line.
              |||,
            },
          ],
        },
        {
          name: 'dependency3',
          version: '3.0.0',
          description: |||
            This is a dependency description.
            This is a second line.
          |||,
          dependencies: [
            {
              name: 'dependency4',
              version: '4.0.0',
              description: |||
                This is a dependency description.
                This is a second line.
              |||,
            },
          ],
        },
      ],
    },
    {
      name: 'server2',
      ip: '10.0.0.2',
    },
  ],
};

// Test for @-prefixed keys quote style: tk uses single quotes '@type':, rtk uses double quotes "@type":
// This is common in Envoy configuration
local envoyConfig = {
  static_resources: {
    listeners: [{
      filter_chains: [{
        filters: [{
          typed_config: {
            '@type': 'type.googleapis.com/envoy.extensions.filters.network.http_connection_manager.v3.HttpConnectionManager',
            http_filters: [{
              typed_config: {
                '@type': 'type.googleapis.com/envoy.extensions.filters.http.router.v3.Router',
              },
            }],
          },
        }],
      }],
    }],
  },
};

// Multi-line HTML content for testing string style (inline quoted with \n vs literal block)
local htmlContent = |||
  <html>
    <head>
      <style>
        body { font-family: sans-serif; }
        .container { margin: 1rem; }
      </style>
    </head>
    <body>
      <h1>Test Page</h1>
      <p class="description">This is a test page with HTML content.</p>
    </body>
  </html>
|||;

{
  configmap: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'app-config',
      namespace: 'default',
    },
    data: {
      'config.yaml': std.manifestYamlDoc(configData),
      'config.json': std.manifestJson(configData),
      'simple.txt': 'Hello, World!',
      'multilineMangled.txt': std.manifestYamlDoc({
        multilineMangled: |||
          This is a multiline string.
          This is a second line. It has an intentional trailing space. tk mangles it. 
        |||,
        otherField: 'otherValue',
      }),
    },
  },
  // Test deeply nested object-in-array indentation (Prometheus alerting rules structure)
  'alerting-rules-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'alerting-rules',
      namespace: 'default',
    },
    data: {
      'alerts.rules': std.manifestYamlDoc(alertingRulesData),
      '12': 'test',
      '12.5': 'test2',
      local ruleset = alertingRulesData.groups[0],
      test: {
        name: ruleset.name,
        // @pokom: If the recording rule has an interval, preserve it, Otherwise set it to empty string so it uses the Mimir default
        [if std.objectHas(ruleset, 'interval') then 'interval']: ruleset.interval,
        rules:
          std.foldl(
            function(acc, fn)
              fn(acc),
            [
              // Manifest the rules as a YAML stream
              function(ruleset)
                std.manifestYamlStream(
                  ruleset,
                  quote_keys=false,
                  c_document_end=false,
                ),
              // TODO(@duologic): enable this after https://qndyjh.jdb/hnuztoc/dptrauqshw_ejnat/lhsn/252255
              //// Split into lines
              //function(manifest)
              //  std.split(manifest, '\n'),
              //// When lines have trailing whitespaces, YAML cannot be rendered as a multi-line string. Remove them.
              //function(lines)
              //  std.map(
              //    function(line) std.rstripChars(line, ' '),
              //    lines,
              //  ),
              //// Join lines again
              //std.lines,
              function(manifest)
                std.strReplace(manifest, ' \n', '\n'),
              // Remove duplicate newlines at the end
              function(manifest)
                std.rstripChars(manifest, '\n') + '\n',
            ],
            std.get(ruleset, 'rules', [])
          ),
      },
    },
  },
  // Test with quote_keys=false (matches real-world usage where keys are unquoted)
  'alerting-rules-unquoted-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'alerting-rules-unquoted',
      namespace: 'default',
    },
    data: {
      // indent_array_in_object=false, quote_keys=false
      'alerts.rules': std.manifestYamlDoc(alertingRulesData, false, false),
    },
  },
  // Test nested YAML indentation in literal block strings
  'nested-yaml-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'nested-yaml-config',
      namespace: 'default',
    },
    spec: {
      templates: [
        {
          name: 'exit-handler',
          steps: [
            [
              {
                arguments: {
                  parameters: [
                    {
                      name: 'title',
                      value: '🚨 Workflow `{{workflow.namespace}}/{{workflow.name}}` failed',
                    },
                  ],
                },
              },
            ],
          ],
        },
      ],
    },
    data: {
      manifestJson1: std.manifestJson({
        test: '\n',
        otherField: 'otherValue',
      }),
      manifestJson3: std.manifestJson({
        otherField: 'otherValue,',
      }),
      manifestJson4: std.manifestJson({
        otherField: '🚀',
      }),
      manifestJson2: std.manifestJsonMinified({
        test: |||

          test
        |||,
        otherField: 'otherValue',
      }),
      manifestYaml1: std.manifestYamlDoc({
        test: '\n',
        otherField: 'otherValue',
      }),
      manifestYaml2: std.manifestYamlDoc({
        test: |||

          test
        |||,
        otherField: 'otherValue',
      }),
      manifestYaml3: std.manifestYamlDoc({
        otherField: 'otherValue,',
      }),
      manifestYaml4: std.manifestYamlDoc({
        otherField: '🚀',
      }),
      test: '🚀',
      test2: 'hello,',
      test3: '2025-11-03T15:00:00',
      test4: [
        [
          {
            value: '🚨 Workflow `{{workflow.namespace}}/{{workflow.name}}` failed',
          },
        ],
      ],
      'rules.json': std.manifestJson(nestedYamlData),
      'rules.json.minified': std.manifestJsonMinified(nestedYamlData),
      'rules.yml': std.manifestYamlDoc(nestedYamlData, quote_keys=false),
      'rules.yml.quoted': std.manifestYamlDoc(nestedYamlData, quote_keys=true),
      'rules.yml.quoted.indented': std.manifestYamlDoc(nestedYamlData, quote_keys=true, indent_array_in_object=true),
    },
  },
  // ConfigMap with dashboard JSON containing floats
  'configmap-dashboard': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'dashboards',
      namespace: 'default',
    },
    data: {
      // Dashboard JSON as string - matching real Grafana dashboard format
      'dashboard.json': import 'dashboard-promtail.json',
      'dashboard-to-string.json': std.toString(import 'dashboard-promtail.json'),
      'dashboard2-to-string.json': std.toString(import 'dashboard-cle-headquarters.json'),
      'dashboard3-to-string.json': std.toString(import 'ds-querier-cluster-deployment.libsonnet'),
      'dashboard4-to-string.json': std.toString(import 'conntrack_exporter.json'),
    },
  },
  // ConfigMap with dashboard JSON containing floats
  'configmap-dashboard-parse-and-output': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'dashboards-parse-and-output',
      namespace: 'default',
    },
    data: {
      // Dashboard JSON as string - matching real Grafana dashboard format
      'dashboard.json': import 'dashboard-promtail.json',
      'dashboard-to-string.json': std.manifestJson(std.parseJson(importstr 'dashboard-promtail.json')),
      'dashboard2-to-string.json': std.manifestJson(std.parseJson(importstr 'dashboard-cle-headquarters.json')),
      'dashboard4-to-string.json': std.manifestJson(std.parseJson(importstr 'conntrack_exporter.json')),
    },
  },
  'configmap-apiserver': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'apiserver',
      namespace: 'default',
    },
    data: {
      name: 'apiserver',
      rules:
        std.foldl(
          function(acc, fn)
            fn(acc),
          [
            // Manifest the rules as a YAML stream
            function(ruleset)
              std.manifestYamlStream(
                ruleset,
                quote_keys=false,
                c_document_end=false,
              ),
            function(manifest)
              std.strReplace(manifest, ' \n', '\n'),
            function(manifest)
              std.rstripChars(manifest, '\n') + '\n',
          ],
          std.get({}, 'rules', [])
        ),
    },
  },
  // Test case for asterisk quoting: tk uses single quotes ('*'), rtk uses double quotes ("*")
  'wildcard-resource': {
    apiVersion: 'example.io/v1',
    kind: 'WildcardConfig',
    metadata: {
      name: 'wildcard-test',
      namespace: 'default',
    },
    spec: {
      // Asterisk wildcard - tk quotes with single quotes, rtk with double quotes
      allowedPrincipal: '*',
      patterns: ['*', 'prefix-*', '*-suffix'],
    },
  },
  // Test multi-line HTML string representation (literal block vs inline with \n)
  'html-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'html-content',
      namespace: 'default',
    },
    data: {
      'index.html': htmlContent,
    },
  },
  cronjob: {
    apiVersion: 'batch/v1',
    kind: 'CronJob',
    metadata: {
      name: 'remove-flux-ignores-before-rollout',
      namespace: 'default',
    },
    spec: {
      schedule: '0 9 * * 1-5',
      jobTemplate: {
        spec: {
          template: {
            spec: {
              containers: [{
                name: 'kubectl',
                image: 'bitnami/kubectl:1.25',
                args: [
                  // This long string triggers different wrapping behavior between tk and rtk
                  '--reason=Removing Flux ignores before scheduled rollout of this cell',
                  '--namespace=metrics-ops-03',
                  '--selector=app.kubernetes.io/name=mimir',
                ],
              }],
            },
          },
        },
      },
    },
  },
  // Test long string wrapping and continuation line indentation
  scaledobject: {
    apiVersion: 'keda.sh/v1alpha1',
    kind: 'ScaledObject',
    metadata: {
      name: 'test-scaled',
      namespace: 'default',
    },
    spec: {
      triggers: [{
        type: 'prometheus',
        metadata: {
          // Long query string that triggers line wrapping - tests continuation indentation
          // tk uses 8-space indent for continuation, rtk uses 4-space
          query: '(1 - (min(kubelet_volume_stats_available_bytes{cluster="test-cluster", namespace="test-ns", persistentvolumeclaim=~"store-gateway-.*"}/kubelet_volume_stats_capacity_bytes{cluster="test-cluster",namespace="test-ns", persistentvolumeclaim=~"store-gateway-.*"}))) * 100',
        },
      }],
    },
  },
  // Test case for Grafana datasources ConfigMap - reproduces grafana-o11y diffs
  // Uses std.manifestYamlDoc with quote_keys=true to match real usage
  'grafana-datasources-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'grafana-datasources',
      namespace: 'default',
    },
    data: {
      [key]: manifestYaml(datasourceConfigs[key])
      for key in std.objectFields(datasourceConfigs)
    },
  },
  // Test case for dashboard provisioning ConfigMap
  'grafana-dashboard-provisioning-configmap': {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'grafana-dashboard-provisioning',
      namespace: 'default',
    },
    data: {
      'dashboards.yml': std.manifestYamlDoc(dashboardProvisioningConfig, quote_keys=true),
    },
  },
  // Test case for long string wrapping behavior (tk wraps at ~80 chars, rtk doesn't)
  deployment: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: 'test-deployment',
      namespace: 'default',
    },
    spec: {
      template: {
        spec: {
          containers: [{
            name: 'test-container',
            // Test empty arrays - tk uses [], rtk uses block style
            env: [],
            volumeMounts: [],
            args: [
              // Long string that should trigger line wrapping
              '-hosted_grafana_api_config=[{"address": "http://btc-nvssqkig.vwk-himevujo.kyc.aotsdcr.gqiel.","slug": "prod-region-0","token": "$(HG_CLUSTER_TOKEN)"}]',
              // Another long string
              'kubectl config set-cluster test-cluster --embed-certs=true --certificate-authority=/var/run/certs/kubernetes/ca.pem.crt --server=https://chljforwj.tzuq.avr.jvdeidd.xreij.:443 --kubeconfig=/var/run/secrets/kubernetes/kubeconfig',
              // String starting with dash containing special chars - tk unquoted, rtk quotes
              '-forward.selectors={__name__="target_info"},{__name__="traces_info"}',
            ],
          }],
          initContainers: [{
            name: 'secrets-copier',
            image: 'busybox:1.34',
            command: ['/bin/sh', '-exc'],
            args: [
              // Long shell command that triggers different line wrapping between tk and rtk
              'chown -R nobody:nobody /var/run/secrets/database;cp /var/run/certs/database/ca.crt /var/run/secrets/database/ca.crt;chown nobody:nobody /var/run/secrets/database/ca.crt;chmod 600 /var/run/secrets/database/ca.crt;cp /var/run/certs/database/client.root.crt /var/run/secrets/database/client.root.crt;chown nobody:nobody /var/run/secrets/database/client.root.crt;chmod 600 /var/run/secrets/database/client.root.crt;cp /var/run/certs/database/client.root.key /var/run/secrets/database/client.root.key;chown nobody:nobody /var/run/secrets/database/client.root.key;chmod 600 /var/run/secrets/database/client.root.key',
            ],
          }],
          // Test empty arrays at pod level
          volumes: [],
        },
      },
    },
  },
}
