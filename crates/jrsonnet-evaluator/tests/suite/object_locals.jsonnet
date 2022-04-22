std.assertEqual({ local a = 3, b: a }, { b: 3 }) &&
std.assertEqual({ local a = 3, local c = a, b: c }, { b: 3 }) &&
std.assertEqual({ local a = function(b) { [b]: 4 }, test: a('test') }, { test: { test: 4 } }) &&
true
