// Method whose body has local + data= object that uses top-level local (like flux environment(clusterName):: data={ ... pkg ... }
local pkg = import 'pkg.libsonnet';

{
  environment(name)::
    local cluster = self.clusters[name];
    pkg.baseEnvironment(
      data={
        x: pkg.doSomething(cluster),
      },
    ),
}
