std.assertEqual({ assert 'a' in self : 'missing a' } + { a: 2 }, { a: 2 }) &&
test.assertThrow({ assert 'a' in self : 'missing a', b: 1 }.b, 'assert failed: missing a') &&
true
