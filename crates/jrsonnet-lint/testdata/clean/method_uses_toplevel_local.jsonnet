// Minimal: top-level local used inside object method body
local pkg = import 'pkg.libsonnet';

{
  local root = self,

  env(name)::
    local x = root.foo;
    pkg.doSomething(x),
}
