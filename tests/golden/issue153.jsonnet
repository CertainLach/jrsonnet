local upper = {
  'bar': if 'baz' in super then super.baz else 'nope',
};

local obj = {
  foo+: upper,
};

obj
