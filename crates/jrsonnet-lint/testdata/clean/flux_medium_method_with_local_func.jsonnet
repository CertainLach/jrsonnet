// Flux-style: top-level locals + method with local function; all used in data= object
local secretBackend = import 'secrets.libsonnet';
local pkgFlux = import 'flux.libsonnet';
local envMeta = import 'env.libsonnet';

{
  local root = self,

  environment(clusterName)::
    local cluster = root.clusters[clusterName];
    local gitRepo(c, secretName, url) = pkgFlux.source.gitRepo(c, secretName, url);
    envMeta.baseEnvironment(
      data={
        secret: secretBackend.map(),
        flux: pkgFlux.new(),
        repo: gitRepo(cluster, 'token', 'https://example.com/repo.git'),
      },
    ),
}
