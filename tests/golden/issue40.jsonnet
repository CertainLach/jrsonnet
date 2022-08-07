local conf = {
  n: '',
};

local result = conf {
  assert std.isNumber(self.n) : 'is number',
};

std.manifestJsonEx(result, '')
