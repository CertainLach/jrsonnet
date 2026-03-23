// Test null-coalesce chained access: a?.b.c should return null when b is missing,
// not fail with "field c not found on null".

local obj = { a: { b: { c: 42 } } };

[
  // null-coalesce on missing field should return null, not error
  obj?.missing.b.c,

  // null-coalesce on present field continues
  obj?.a.b.c,

  // null-coalesce with bracket index
  obj?.["missing"].b.c,

  // chained null-coalesce
  obj?.a?.missing.c,
]
