// Inner local defined but never used inside the method body
local outer = 10;
{
  method()::
    local inner = 5;
    outer,
}
