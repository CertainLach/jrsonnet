// objectRemoveKey should retain hidden fields as hidden fields.
std.assertEqual(std.objectRemoveKey({ foo: 1, bar: 2, baz:: 3 }, 'foo').baz, 3) &&
// objectRemoveKey doesn't break inheritance within the provided object.
std.assertEqual(std.objectRemoveKey({ a: 1 } + { b: super.a }, 'a'), { b: 1 }) &&
// objectRemoveKey works with inheritance outside of the object.
std.assertEqual({ a: 1 } + std.objectRemoveKey({ b: super.a }, 'a'), { a: 1, b: 1 }) &&
// Referential transparency still works.
std.assertEqual(local o1 = { b: super.a }; std.objectRemoveKey({ a: 1 } + o1, 'a'), { b: 1 }) &&
std.assertEqual(local o1 = { b: super.a }; { a: 1 } + std.objectRemoveKey(o1, 'a'), { a: 1, b: 1 }) &&
// Hidden fields still work.
std.assertEqual(std.objectRemoveKey({ a: 1 } + { b:: super.a }, 'a'), {}) &&
std.assertEqual(std.objectRemoveKey({ a: 1 } + { b:: super.a }, 'a').b, 1) &&
std.assertEqual(({ a: 1 } + std.objectRemoveKey({ b:: super.a }, 'a')), { a: 1 }) &&
std.assertEqual(({ a: 1 } + std.objectRemoveKey({ b:: super.a }, 'a')).b, 1) &&
true
