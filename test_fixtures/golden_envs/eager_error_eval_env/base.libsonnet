// Simulates thor-query-engine.libsonnet
{
  _config+:: {
    thor_enabled: true,
    loki+:: if $._config.thor_enabled then {
      querier+: {
        dataobj_storage_start: error 'dataobj_storage_start must be set',
        engine+: {
          storage_start: $._config.loki.querier.dataobj_storage_start,
        },
        normal: 'value',
      },
    } else {},
  },
}
