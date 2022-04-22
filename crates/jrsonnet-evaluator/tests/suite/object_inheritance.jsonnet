std.assertEqual({ a: self.b } + { b: 3 }, { a: 3, b: 3 }) &&
std.assertEqual(
  {
    name: 'Alice',
    welcome: 'Hello ' + self.name + '!',
  },
  { name: 'Alice', welcome: 'Hello Alice!' },
) &&
std.assertEqual(
  {
    name: 'Alice',
    welcome: 'Hello ' + self.name + '!',
  } + {
    name: 'Bob',
  }, { name: 'Bob', welcome: 'Hello Bob!' }
) &&
true
