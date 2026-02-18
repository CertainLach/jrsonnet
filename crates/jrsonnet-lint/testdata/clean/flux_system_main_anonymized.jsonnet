// Anonymized from environments/flux-system/main.jsonnet
// Structure: top-level locals + object with method returning env; method body uses all locals in data= and computed keys.
local secretBackend = import 'external-secrets/secrets.libsonnet';
local pkgFlux = import 'flux2/helm.libsonnet';
local appSecretWriter = import 'github-app-secret-writer/main.libsonnet';
local kutil = import 'ksonnet-util/kausal.libsonnet';
local clustersMeta = import 'meta/clusters.libsonnet';
local nsList = import 'meta/namespaces.libsonnet';
local appsByCluster = import 'meta/raw/apps-by-cluster.json';
local team = import 'meta/teams/productivity.libsonnet';
local envMeta = import 'meta/v1alpha1/environment.libsonnet';
local migrationPhases = import 'oci-migration-phases.libsonnet';

{
  local root = self,
  namespace:: 'app-system',
  app:: root.namespace,

  clusters::
    clustersMeta.by_featureset.primary.all
    + clustersMeta.clustersBeingDecommissioned
  ,

  local waveSelector = 'waveV2',

  environment(clusterName)::
    local cluster = root.clusters[clusterName];
    local fluxcdLib = import 'fluxcd/main.libsonnet';
    local fluxLib = fluxcdLib[cluster[waveSelector]];
    local clusterHasObservabilityPlatform = std.member(std.get(appsByCluster, clusterName, []), 'observability-platform');

    local gitRepo(cluster, fluxDeployKeysSecretName, cloneUrl) =
      local name = cluster.cluster_name;
      fluxLib.source.gitRepository.new('repo-manifests')
      + fluxLib.source.gitRepository.spec.withUrl(cloneUrl)
      + fluxLib.source.gitRepository.spec.withInterval('1m0s')
      + fluxLib.source.gitRepository.spec.ref.withBranch('master')
      + fluxLib.source.gitRepository.spec.secretRef.withName(fluxDeployKeysSecretName)
      + fluxLib.source.gitRepository.spec.withIgnore(||| /* */ !/flux/%(name)s ||| % { name: name })
      + (
        if std.member(migrationPhases.phase3OrLater, name)
        then fluxLib.source.gitRepository.spec.withSuspend(true)
        else {}
      )
    ;

    local ociRepositoryApi =
      if std.objectHas(fluxLib, 'sourceV1beta2')
      then fluxLib.sourceV1beta2
      else fluxLib.source;
    local ocirepository = ociRepositoryApi.ociRepository;

    local ociRepoForCluster(cluster) =
      local name = cluster.cluster_name;
      local imageRef = 'oci://us-docker.pkg.dev/proj/repo-%s/%s' % [name, name];
      ocirepository.new('repo-manifests-oci')
      + ocirepository.metadata.withNamespace(root.namespace)
      + ocirepository.spec.withUrl(imageRef)
      + ocirepository.spec.withInterval('1m0s')
      + ocirepository.spec.ref.withTag('master')
      + ocirepository.spec.secretRef.withName('registry')
    ;

    local ociRepoForObservabilityPlatform(cluster) =
      local name = cluster.cluster_name;
      local imageRef = 'oci://us-docker.pkg.dev/proj/repo-%s/observability-%s' % [name, name];
      ocirepository.new('repo-manifests-oci-observability')
      + ocirepository.metadata.withNamespace(root.namespace)
      + ocirepository.spec.withUrl(imageRef)
      + ocirepository.spec.withInterval('1m0s')
      + ocirepository.spec.ref.withTag('master')
      + ocirepository.spec.secretRef.withName('registry')
    ;

    envMeta.baseEnvironment(
      root.app,
      cluster,
      root.namespace,
      data={
        local basePath = 'flux/%(cluster_name)s' % { cluster_name: clusterName },
        local fluxDeployKeysSecretName = 'flux-app-token',

        [if !std.member(migrationPhases.phase4OrLater, clusterName) then 'appSecretWriter']:
          appSecretWriter(
            'flux-app-writer',
            clusterWave=cluster[waveSelector],
            namespace=root.namespace,
            targetSecretName=fluxDeployKeysSecretName,
            appSecret='flux-app',
            pullSecret=self.secrets.registry,
            targetSecretKey='password',
            targetSecretUsernameKey='username'
          ),

        local karpenterTolerationsMixin(c) =
          local tol = kutil.core.v1.toleration.withKey('CriticalAddonsOnly') + kutil.core.v1.toleration.withOperator('Exists');
          pkgFlux.withKustomizeControllerTolerations(tol)
          + pkgFlux.withSourceControllerTolerations(tol)
        ,

        local eksPreferredAffinity() =
          local aff = kutil.core.v1.affinity.nodeAffinity.withPreferredDuringSchedulingIgnoredDuringExecution(
            kutil.core.v1.preferredSchedulingTerm.withWeight(1)
            + kutil.core.v1.preferredSchedulingTerm.preference.withMatchExpressions([{ key: 'CriticalAddonsOnly', operator: 'Exists' }])
          );
          pkgFlux.withSourceControllerAffinity(aff)
          + pkgFlux.withKustomizeControllerAffinity(aff),

        local waves = import 'waves.libsonnet',
        local wave = std.get(waves, cluster.cluster_name, waves[cluster[waveSelector]]),
        local kustomizeControllerResourcesByCluster = { default: { cpu: '4', memory: '5.75Gi' }, default_pop: { cpu: '1', memory: '512Mi' } },
        local sourceControllerResourcesByCluster = { default: { cpu: '2', memory: '1.5Gi' }, default_pop: { cpu: '1.5', memory: '3Gi' } },

        local resourcesForClusterWithDefault(resourcesForCluster, c) =
          local default = if c.next_gen_pop_cluster then resourcesForCluster.default_pop else resourcesForCluster.default;
          std.get(resourcesForCluster, c.cluster_name, default)
        ,

        local kustomizeControllerResources = resourcesForClusterWithDefault(kustomizeControllerResourcesByCluster, cluster),
        local sourceControllerResources = resourcesForClusterWithDefault(sourceControllerResourcesByCluster, cluster),

        fluxcd:
          pkgFlux.new(
            root.namespace,
            chartVersion=wave,
            installCRDs=true,
            watchallnamespaces=true,
            mixin=
            (
              pkgFlux.withKustomizeControllerResources(requests=kustomizeControllerResources)
              + pkgFlux.withSourceControllerResources(requests=sourceControllerResources)
              + pkgFlux.withNotificationControllerResources(requests={ cpu: '100m', memory: '128Mi' })
              + (
                if cluster.provider == 'eks'
                then karpenterTolerationsMixin(cluster) + eksPreferredAffinity()
                else {}
              )
            )
          )
          + pkgFlux.withSourceControllerMemoryMounts()
          + pkgFlux.withKustomizeControllerMemoryMounts()
          + (if std.member(['dev0'], cluster.waveV2) then pkgFlux.withoutKustomizeControllerLeaderElection() else {}),

        [if !std.member(migrationPhases.phase4OrLater, clusterName) then 'gitRepo']:
          gitRepo(cluster, fluxDeployKeysSecretName, cloneUrl='https://github.com/org/repo-manifests.git'),

        [if std.member(migrationPhases.phase1OrLater + migrationPhases.phase4OrLater, clusterName) then 'ocirepository']:
          ociRepoForCluster(cluster)
          + ocirepository.spec.withSuspend(!std.member(migrationPhases.phase2OrLater + migrationPhases.phase4OrLater, clusterName)),

        [if std.member(migrationPhases.phase1OrLater + migrationPhases.phase4OrLater, clusterName) && clusterHasObservabilityPlatform then 'ocirepository_observability']:
          ociRepoForObservabilityPlatform(cluster)
          + ocirepository.spec.withSuspend(!std.member(migrationPhases.phase2OrLater + migrationPhases.phase4OrLater, clusterName)),

        local secretName = 'app-new',
        secrets:
          {
            registry: secretBackend.mapGARSecret(),
          }
          + (
            if !std.member(migrationPhases.phase4OrLater, clusterName)
            then {
              github_app: secretBackend.mapExternalSecret(
                'flux-app',
                '%s/%s' % [root.namespace, secretName],
              ),
            }
            else {}
          ),
      },
    )
    + envMeta.withTeam(team)
    + envMeta.withExportClusterWideAlongsideNamespaced(root.namespace)
    + envMeta.withNamespaces(nsList),

  envs: envMeta.generateEnvs(root.clusters, root.environment),
}
