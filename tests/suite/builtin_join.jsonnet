std.assertEqual(std.join([0, 0], [[1, 2], [3, 4], [5, 6]]), [1, 2, 0, 0, 3, 4, 0, 0, 5, 6]) &&
std.assertEqual(std.join(',', ['1', '2', '3', '4']), '1,2,3,4') &&
std.assertEqual(std.join(',', ['1', null, '2', null, '3']), '1,2,3') &&
true
