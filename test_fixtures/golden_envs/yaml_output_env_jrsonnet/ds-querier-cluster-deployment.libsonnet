local filename = 'ds-querier-cluster-deployment.json';

{
  [filename]: {
    annotations: {
      list: [
        {
          builtIn: 1,
          datasource: {
            type: 'grafana',
            uid: '-- Grafana --',
          },
          enable: true,
          hide: true,
          iconColor: 'rgba(0, 211, 255, 1)',
          name: 'Annotations & Alerts',
          target: {
            limit: 100,
            matchAny: false,
            tags: [
              '$cluster',
            ],
            type: 'tags',
          },
          type: 'dashboard',
        },
      ],
    },
    editable: true,
    graphTooltip: 0,
    links: [],
    panels: [
      {
        datasource: {
          type: 'prometheus',
          uid: 'metrics-ops-03',
        },
        fieldConfig: {
          defaults: {
            color: {
              mode: 'palette-classic',
            },
            custom: {
              axisBorderShow: false,
              axisCenteredZero: false,
              axisColorMode: 'text',
              axisLabel: '',
              axisPlacement: 'auto',
              barAlignment: 0,
              barWidthFactor: 0.6,
              drawStyle: 'line',
              fillOpacity: 0,
              gradientMode: 'none',
              hideFrom: {
                legend: false,
                tooltip: false,
                viz: false,
              },
              insertNulls: false,
              lineInterpolation: 'linear',
              lineWidth: 1,
              pointSize: 5,
              scaleDistribution: {
                type: 'linear',
              },
              showPoints: 'auto',
              spanNulls: false,
              stacking: {
                group: 'A',
                mode: 'none',
              },
              thresholdsStyle: {
                mode: 'off',
              },
            },
            mappings: [],
            thresholds: {
              mode: 'absolute',
              steps: [
                {
                  color: 'green',
                  value: null,
                },
                {
                  color: 'red',
                  value: 80,
                },
              ],
            },
          },
          overrides: [],
        },
        gridPos: {
          h: 8,
          w: 12,
          x: 0,
          y: 0,
        },
        id: 1,
        options: {
          legend: {
            calcs: [],
            displayMode: 'list',
            placement: 'bottom',
            showLegend: true,
          },
          tooltip: {
            hideZeros: false,
            mode: 'single',
            sort: 'none',
          },
        },
        pluginVersion: '11.6.0-82015',
        targets: [
          {
            datasource: {
              type: 'prometheus',
              uid: 'metrics-ops-03',
            },
            editorMode: 'code',
            expr: 'sum by(cluster) (grafana_feature_toggles_info{cluster="$cluster", name="mtQuerierEnabled"})',
            legendFormat: '__auto',
            range: true,
            refId: 'A',
          },
        ],
        title: 'Instances with mtQuerierEnabled',
        transparent: true,
        type: 'timeseries',
      },
      {
        datasource: {
          type: 'prometheus',
          uid: 'ops-cortex',
        },
        description: 'Not every 500/400 here is necessarily a new error. ',
        fieldConfig: {
          defaults: {
            color: {
              mode: 'palette-classic',
            },
            custom: {
              axisBorderShow: false,
              axisCenteredZero: false,
              axisColorMode: 'text',
              axisLabel: '',
              axisPlacement: 'auto',
              barAlignment: 0,
              barWidthFactor: 0.6,
              drawStyle: 'line',
              fillOpacity: 100,
              gradientMode: 'none',
              hideFrom: {
                legend: false,
                tooltip: false,
                viz: false,
              },
              insertNulls: false,
              lineInterpolation: 'linear',
              lineWidth: 0,
              pointSize: 5,
              scaleDistribution: {
                type: 'linear',
              },
              showPoints: 'never',
              spanNulls: false,
              stacking: {
                group: 'A',
                mode: 'normal',
              },
              thresholdsStyle: {
                mode: 'off',
              },
            },
            mappings: [],
            min: 0,
            thresholds: {
              mode: 'absolute',
              steps: [
                {
                  color: 'green',
                  value: null,
                },
                {
                  color: 'red',
                  value: 80,
                },
              ],
            },
            unit: 'short',
          },
          overrides: [
            {
              matcher: {
                id: 'byName',
                options: '5xx',
              },
              properties: [
                {
                  id: 'color',
                  value: {
                    fixedColor: 'dark-red',
                    mode: 'fixed',
                  },
                },
              ],
            },
            {
              matcher: {
                id: 'byName',
                options: '4xx',
              },
              properties: [
                {
                  id: 'color',
                  value: {
                    fixedColor: 'dark-yellow',
                    mode: 'fixed',
                  },
                },
              ],
            },
            {
              matcher: {
                id: 'byName',
                options: '2xx',
              },
              properties: [
                {
                  id: 'color',
                  value: {
                    fixedColor: 'dark-green',
                    mode: 'fixed',
                  },
                },
              ],
            },
          ],
        },
        gridPos: {
          h: 8,
          w: 12,
          x: 12,
          y: 0,
        },
        id: 2,
        options: {
          legend: {
            calcs: [],
            displayMode: 'list',
            placement: 'bottom',
            showLegend: true,
          },
          tooltip: {
            hideZeros: false,
            mode: 'multi',
            sort: 'desc',
          },
        },
        pluginVersion: '11.6.0-82015',
        targets: [
          {
            datasource: {
              type: 'prometheus',
              uid: 'ops-cortex',
            },
            editorMode: 'code',
            expr: 'sum by (status) (\n  label_replace(label_replace(rate(mtgr_grafana_alerting_querier_requests_total{cluster=~"$cluster", job=~"(grafana-ruler)/grafana-ruler", operation="apis_query_grafana_app_v0alpha1_namespaces_stack_query"}[$__rate_interval]),\n  "status", "${1}xx", "status_code", "([0-9]).."),\n  "status", "${1}", "status_code", "([a-zA-Z]+)"))\n',
            legendFormat: '__auto',
            range: true,
            refId: 'A',
          },
        ],
        title: 'ds-querier reqs/sec',
        transparent: true,
        type: 'timeseries',
      },
      {
        datasource: {
          type: 'prometheus',
          uid: 'metrics-ops-03',
        },
        description: 'The red line is our north star',
        fieldConfig: {
          defaults: {
            color: {
              mode: 'palette-classic',
            },
            custom: {
              axisBorderShow: false,
              axisCenteredZero: false,
              axisColorMode: 'text',
              axisLabel: '',
              axisPlacement: 'auto',
              barAlignment: 0,
              barWidthFactor: 0.6,
              drawStyle: 'line',
              fillOpacity: 10,
              gradientMode: 'none',
              hideFrom: {
                legend: false,
                tooltip: false,
                viz: false,
              },
              insertNulls: false,
              lineInterpolation: 'linear',
              lineWidth: 1,
              pointSize: 5,
              scaleDistribution: {
                type: 'linear',
              },
              showPoints: 'auto',
              spanNulls: false,
              stacking: {
                group: 'A',
                mode: 'none',
              },
              thresholdsStyle: {
                mode: 'off',
              },
            },
            mappings: [],
            min: 0,
            thresholds: {
              mode: 'absolute',
              steps: [
                {
                  color: 'green',
                  value: null,
                },
                {
                  color: 'red',
                  value: 80,
                },
              ],
            },
            unit: 'short',
          },
          overrides: [
            {
              matcher: {
                id: 'byName',
                options: 'failed',
              },
              properties: [
                {
                  id: 'color',
                  value: {
                    fixedColor: 'dark-red',
                    mode: 'fixed',
                  },
                },
              ],
            },
            {
              matcher: {
                id: 'byName',
                options: 'successful',
              },
              properties: [
                {
                  id: 'color',
                  value: {
                    fixedColor: 'green',
                    mode: 'fixed',
                  },
                },
              ],
            },
          ],
        },
        gridPos: {
          h: 8,
          w: 12,
          x: 0,
          y: 8,
        },
        id: 4,
        options: {
          legend: {
            calcs: [],
            displayMode: 'list',
            placement: 'bottom',
            showLegend: true,
          },
          tooltip: {
            hideZeros: false,
            mode: 'multi',
            sort: 'desc',
          },
        },
        pluginVersion: '11.6.0-82015',
        targets: [
          {
            editorMode: 'code',
            expr: 'sum(rate(mtgr_grafana_alerting_rule_evaluations_total{cluster=~"$cluster", job=~"grafana-ruler/grafana-ruler"}[$__rate_interval]))\n-\nsum(rate(mtgr_grafana_alerting_rule_evaluation_failures_total{cluster=~"$cluster", job=~"grafana-ruler/grafana-ruler"}[$__rate_interval]))\n',
            legendFormat: 'successful',
            range: true,
            refId: 'A',
          },
          {
            datasource: {
              type: 'prometheus',
              uid: 'metrics-ops-03',
            },
            editorMode: 'code',
            expr: 'sum(rate(mtgr_grafana_alerting_rule_evaluation_failures_total{cluster=~"$cluster", job=~"grafana-ruler/grafana-ruler"}[$__rate_interval]))',
            hide: false,
            instant: false,
            legendFormat: 'failed',
            range: true,
            refId: 'B',
          },
        ],
        title: 'Ruler evals / sec 🌟',
        transparent: true,
        type: 'timeseries',
      },
      {
        datasource: {
          type: 'prometheus',
          uid: 'ops-cortex',
        },
        description: '',
        fieldConfig: {
          defaults: {
            color: {
              mode: 'palette-classic',
            },
            custom: {
              axisBorderShow: false,
              axisCenteredZero: false,
              axisColorMode: 'text',
              axisLabel: '',
              axisPlacement: 'auto',
              barAlignment: 0,
              barWidthFactor: 0.6,
              drawStyle: 'line',
              fillOpacity: 0,
              gradientMode: 'none',
              hideFrom: {
                legend: false,
                tooltip: false,
                viz: false,
              },
              insertNulls: false,
              lineInterpolation: 'linear',
              lineWidth: 1,
              pointSize: 5,
              scaleDistribution: {
                type: 'linear',
              },
              showPoints: 'auto',
              spanNulls: false,
              stacking: {
                group: 'A',
                mode: 'none',
              },
              thresholdsStyle: {
                mode: 'off',
              },
            },
            mappings: [],
            thresholds: {
              mode: 'absolute',
              steps: [
                {
                  color: 'green',
                  value: null,
                },
                {
                  color: 'red',
                  value: 80,
                },
              ],
            },
            unit: 'percentunit',
          },
          overrides: [
            {
              __systemRef: 'hideSeriesFrom',
              matcher: {
                id: 'byNames',
                options: {
                  mode: 'exclude',
                  names: [
                    'prod-region-2-app-frontend-api-mysql8',
                  ],
                  prefix: 'All except:',
                  readOnly: true,
                },
              },
              properties: [
                {
                  id: 'custom.hideFrom',
                  value: {
                    legend: false,
                    tooltip: false,
                    viz: true,
                  },
                },
              ],
            },
          ],
        },
        gridPos: {
          h: 8,
          w: 12,
          x: 12,
          y: 8,
        },
        id: 7,
        options: {
          legend: {
            calcs: [],
            displayMode: 'list',
            placement: 'right',
            showLegend: true,
          },
          tooltip: {
            hideZeros: false,
            mode: 'multi',
            sort: 'desc',
          },
        },
        pluginVersion: '11.6.0-82015',
        targets: [
          {
            datasource: {
              type: 'prometheus',
              uid: 'ops-cortex',
            },
            editorMode: 'code',
            expr: 'max(stackdriver_cloudsql_database_cloudsql_googleapis_com_database_cpu_utilization{database_id=~".*$cluster-(hg|hosted\\\\-grafana)+-(api|shared)+.*"}) by (database_id)',
            hide: false,
            instant: false,
            key: 'Q-4df91620-56e5-41d5-bc41-4187c456e8c6-0',
            legendFormat: '{{database_id}}',
            range: true,
            refId: 'GCP',
          },
          {
            datasource: {
              type: 'prometheus',
              uid: 'ops-cortex',
            },
            editorMode: 'code',
            expr: 'max(\n    azure_mysql_server_cpu_percent{instance_name=~"$cluster-app-frontend-.*-mysql8.*|$cluster-app-frontend.*-api.*|$cluster-mysql-\\\\d-.*"}\n) by (instance_name)\n/ 100',
            hide: false,
            instant: false,
            legendFormat: '{{instance_name}}',
            range: true,
            refId: 'AZURE',
          },
          {
            datasource: {
              type: 'prometheus',
              uid: 'ops-cortex',
            },
            disableTextWrap: false,
            editorMode: 'code',
            expr: 'max(aws_rds_cpuutilization_average{job=~"yet-another-cloudwatch-exporter.+", dimension_DBInstanceIdentifier=~"$cluster-(hg|hosted\\\\-grafana)+-(api|shared)+.*"} / 100) by (dimension_DBInstanceIdentifier)',
            format: 'time_series',
            fullMetaSearch: false,
            hide: false,
            includeNullMetadata: true,
            instant: false,
            key: 'Q-4df91620-56e5-41d5-bc41-4187c456e8c6-0',
            legendFormat: '{{dimension_DBInstanceIdentifier}}',
            range: true,
            refId: 'AWS',
            useBackend: false,
          },
        ],
        title: 'HGAPI DB CPU Utilization',
        transparent: true,
        type: 'timeseries',
      },
      {
        datasource: {
          type: 'prometheus',
          uid: 'metrics-ops-03',
        },
        description: 'More zoomed in view of the 🌟',
        fieldConfig: {
          defaults: {
            color: {
              mode: 'palette-classic',
            },
            custom: {
              axisBorderShow: false,
              axisCenteredZero: false,
              axisColorMode: 'text',
              axisLabel: '',
              axisPlacement: 'auto',
              barAlignment: 0,
              barWidthFactor: 0.6,
              drawStyle: 'line',
              fillOpacity: 0,
              gradientMode: 'none',
              hideFrom: {
                legend: false,
                tooltip: false,
                viz: false,
              },
              insertNulls: false,
              lineInterpolation: 'linear',
              lineWidth: 1,
              pointSize: 5,
              scaleDistribution: {
                type: 'linear',
              },
              showPoints: 'auto',
              spanNulls: false,
              stacking: {
                group: 'A',
                mode: 'none',
              },
              thresholdsStyle: {
                mode: 'off',
              },
            },
            mappings: [],
            thresholds: {
              mode: 'absolute',
              steps: [
                {
                  color: 'green',
                  value: null,
                },
                {
                  color: 'red',
                  value: 80,
                },
              ],
            },
          },
          overrides: [],
        },
        gridPos: {
          h: 8,
          w: 24,
          x: 0,
          y: 16,
        },
        id: 6,
        options: {
          legend: {
            calcs: [],
            displayMode: 'list',
            placement: 'bottom',
            showLegend: false,
          },
          tooltip: {
            hideZeros: false,
            mode: 'single',
            sort: 'none',
          },
        },
        pluginVersion: '11.6.0-82015',
        targets: [
          {
            datasource: {
              type: 'prometheus',
              uid: 'metrics-ops-03',
            },
            editorMode: 'code',
            expr: 'sum(rate(mtgr_grafana_alerting_rule_evaluation_failures_total{cluster=~"$cluster", job=~"grafana-ruler/grafana-ruler"}[$__rate_interval]))\n',
            legendFormat: '__auto',
            range: true,
            refId: 'A',
          },
        ],
        title: 'Ruler: error rate',
        transparent: true,
        type: 'timeseries',
      },
      {
        datasource: {
          type: 'loki',
          uid: 'loki-ops',
        },
        fieldConfig: {
          defaults: {},
          overrides: [],
        },
        gridPos: {
          h: 8,
          w: 24,
          x: 0,
          y: 24,
        },
        id: 3,
        options: {
          dedupStrategy: 'none',
          enableInfiniteScrolling: false,
          enableLogDetails: true,
          prettifyLogMessage: false,
          showCommonLabels: false,
          showLabels: false,
          showTime: false,
          sortOrder: 'Descending',
          wrapLogMessage: false,
        },
        pluginVersion: '11.6.0-82015',
        targets: [
          {
            datasource: {
              type: 'loki',
              uid: 'loki-ops',
            },
            editorMode: 'code',
            expr: '{container="grafana-ruler", cluster="$cluster"} |= "Failed to evaluate rule"',
            queryType: 'range',
            refId: 'A',
          },
        ],
        title: 'Ruler: Failed to evaluate rule',
        transparent: true,
        type: 'logs',
      },
      {
        datasource: {
          type: 'loki',
          uid: 'loki-ops',
        },
        fieldConfig: {
          defaults: {},
          overrides: [],
        },
        gridPos: {
          h: 8,
          w: 24,
          x: 0,
          y: 32,
        },
        id: 5,
        options: {
          dedupStrategy: 'none',
          enableInfiniteScrolling: false,
          enableLogDetails: true,
          prettifyLogMessage: false,
          showCommonLabels: false,
          showLabels: false,
          showTime: false,
          sortOrder: 'Descending',
          wrapLogMessage: false,
        },
        pluginVersion: '11.5.0-81938',
        targets: [
          {
            datasource: {
              type: 'loki',
              uid: 'loki-ops',
            },
            editorMode: 'code',
            expr: '{namespace="grafana-datasources", container=~"query-grafana-app.*", cluster="$cluster"} ',
            queryType: 'range',
            refId: 'A',
          },
        ],
        title: 'ds-querier logs',
        transparent: true,
        type: 'logs',
      },
    ],
    preload: false,
    refresh: '1m',
    schemaVersion: 40,
    tags: [
      'core-services-squad',
      'ds-querier',
      'query-service',
      'grafana-datasources',
    ],
    templating: {
      list: [
        {
          allValue: '.+',
          current: {
            text: 'prod-region-2',
            value: 'prod-region-2',
          },
          definition: 'label_values(mtgr_build_info,cluster)',
          includeAll: false,
          name: 'cluster',
          options: [],
          query: {
            qryType: 1,
            query: 'label_values(mtgr_build_info,cluster)',
            refId: 'PrometheusVariableQueryEditor-VariableQuery',
          },
          refresh: 1,
          regex: '',
          sort: 2,
          type: 'query',
        },
      ],
    },
    time: {
      from: 'now-1h',
      to: 'now',
    },
    timepicker: {},
    timezone: 'utc',
    title: 'Grafana Datasources / ds-querier cluster deployment',
    uid: std.md5(filename),
    version: 0,
  },
}
