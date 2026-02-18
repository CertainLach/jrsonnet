// Top-level local x is shadowed by object local x; outer x is never used
local x = 'outer';
{
  local x = 'inner',
  field: x,
}
