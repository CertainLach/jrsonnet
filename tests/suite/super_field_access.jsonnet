// 1. +: works without matching super field
local obj1 = { field+: { foo: 123 } };
std.assertEqual(obj1, { field: { foo: 123 } }) &&

// 2. +: merges with existing super field
local base2 = { field: { a: 1 } };
local child2 = base2 { field+: { b: 2 } };
std.assertEqual(child2, { field: { a: 1, b: 2 } }) &&

// 3. super.field works when field exists
local base3 = { a: 1 };
local child3 = base3 { b: super.a + 10 };
std.assertEqual(child3, { a: 1, b: 11 }) &&

// 4. +: chains work across multiple inheritance layers
local l1 = { items+: ['a'] };
local l2 = l1 { items+: ['b'] };
local l3 = l2 { items+: ['c'] };
std.assertEqual(l3.items, ['a', 'b', 'c']) &&

// 5. +: with objects across multiple layers
local o1 = { cfg+: { x: 1 } };
local o2 = o1 { cfg+: { y: 2 } };
local o3 = o2 { cfg+: { z: 3 } };
std.assertEqual(o3.cfg, { x: 1, y: 2, z: 3 }) &&

// 6. "in super" returns false for missing fields (no error)
local base6 = { a: 1 };
local child6 = base6 { b: if 'nonexistent' in super then super.nonexistent else 'default' };
std.assertEqual(child6.b, 'default') &&

// 7. Mixin pattern: +: on hidden field without super
local mixin = { _config+:: { extra: true } };
local combined = { _config:: { base: true } } + mixin;
std.assertEqual(combined._config, { base: true, extra: true }) &&

true
