local fun(a) = 2;
std.assertEqual(fun(error '3'), 2) &&
// But in tailstrict mode arguments are evaluated eagerly
test.assertThrow(fun(error '3') tailstrict, 'runtime error: 3') &&
true
