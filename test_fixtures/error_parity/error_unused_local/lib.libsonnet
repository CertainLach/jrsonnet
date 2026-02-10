{
  test:: error 'test',
  test2: if $._config.enabled then error 'my error' else 'no error',
  _config:: {
    enabled: true,
  },
}
