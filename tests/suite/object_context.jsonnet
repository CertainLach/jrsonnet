// `self` assigned to `me` was lost when being
// referenced from field
std.assertEqual({
  local me = self,
  a: 3,
  b: me.a,
}.b, 3) &&
std.assertEqual({
  local me = self,
  a: 3,
  b(): me.a,
}.b(), 3) &&
true
