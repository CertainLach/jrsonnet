std.assertEqual(local a = 2; local b = 3; a + b, 5) &&
std.assertEqual(local a = 1, b = a + 1; a + b, 3) &&
std.assertEqual(local a = 1; local a = 2; a, 2) &&
true
