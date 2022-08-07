local k = {
  t(name=self.h): [self.h, name],
  h: 3,
};
local f = {
  t: k.t(),
  h: 4,
};
std.assertEqual(f.t[0], f.t[1]) &&
true
