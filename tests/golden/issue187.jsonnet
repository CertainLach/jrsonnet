std.decodeUTF8(std.encodeUTF8('foo bar ') + [255] + std.encodeUTF8(' baz'))
