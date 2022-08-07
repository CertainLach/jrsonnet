local c = '😎';
std.assertEqual({ c: std.codepoint(c), l: std.length(c) }, { c: 128526, l: 1 }) &&
true
