!std.member('', '') &&
std.member('abc', 'a') &&
!std.member('abc', 'd') &&
!std.member([], '') &&
std.member(['a', 'b', 'c'], 'a') &&
!std.member(['a', 'b', 'c'], 'd') &&
true
