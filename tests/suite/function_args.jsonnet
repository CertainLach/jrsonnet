std.assertEqual(local a = function(b, c=2) b + c; a(2), 4) &&
std.assertEqual(local a = function(b, c='Dear') b + c + d, d = 'World'; a('Hello'), 'HelloDearWorld') &&
true
