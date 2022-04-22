local std2 = std; local std = std2 { primitiveEquals(a, b):: false };
// In jsonnet, this expression was failing because of being desugared to std.primitiveEquals(1, 1)
std.assertEqual(1 == 1, true)
