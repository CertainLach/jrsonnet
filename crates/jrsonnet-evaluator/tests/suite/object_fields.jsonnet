local a = 'a', b = null;
std.assertEqual({ [a]: 2 }, { a: 2 }) &&
std.assertEqual({ [b]: 2 }, {}) &&
true
