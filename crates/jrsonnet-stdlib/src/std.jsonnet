{
  local std = self,

  thisFile:: error 'std.thisFile is deprecated, to enable its support in jrsonnet - recompile it with "legacy-this-file" support.\nThis will slow down stdlib caching a bit, though',
}
