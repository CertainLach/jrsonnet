std.assertEqual(std.objectFields({
  a: {
    [name]: name
    for name in std.objectFields(self)
  },
  b: 2,
  c: 3,
}.a), ['a', 'b', 'c'])
