local obj = {
  a: 1,
  b: 2,
  c: 3,
};
local test = obj + {
  fields: std.objectFields(super),
  d: 5,
};
std.assertEqual(test.fields, ['a', 'b', 'c']) &&
true
