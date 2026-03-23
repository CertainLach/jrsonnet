// Regression test: chained index a.b.c.d should produce a single
// Index { a, [b, c, d] } not nested Index nodes.
// This matters for exp-null-coaelse where a?.b.c should skip .c if .b is null.

local obj = { a: { b: { c: 42 } } };

[
  obj.a.b.c,
  {a: {b: 1}}.a.b,
]
