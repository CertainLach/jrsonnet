{
  length: std.length([1, 2, 3]),
  type: std.type('hello'),
  format: std.format('Hello, %s!', ['world']),
  manifest: std.manifestJsonEx({ foo: 'bar' }, '  '),
}
