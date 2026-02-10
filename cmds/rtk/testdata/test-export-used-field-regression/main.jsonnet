local cfg = ({ assert self.used > 0 : 'used must be positive' } + { used: 1 });

{
  config: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: 'used-field-regression',
    },
    data: {
      used: std.toString(cfg.used),
    },
  },
}
