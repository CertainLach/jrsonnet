// Minimal flux-style: top-level local used only in method body inside data= object
local pkg = import 'pkg.libsonnet';
local envMeta = import 'env.libsonnet';

{
  local root = self,
  namespace:: 'ns',

  environment(name)::
    envMeta.baseEnvironment(
      data={
        key: pkg.foo(root.namespace),
      },
    ),
}
