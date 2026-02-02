// Test that assertions in hidden fields are triggered when accessed
// This should fail with assertion error when $._config.attr is accessed
{
  my_resource: {
    kind: 'ConfigMap',
    apiVersion: 'v1',
    metadata: {
      name: 'test',
      namespace: 'default',
    },
    data: {
      attr: $._config.attr,
    },
  },
  _config:: {
    attr: 'hello',
    assert false : 'should be asserted',
  },
}
