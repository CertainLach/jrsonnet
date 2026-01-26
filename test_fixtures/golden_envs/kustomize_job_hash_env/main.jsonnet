// Test case: kustomize build with job name hashing (mimics k6-cloud pattern)
// This tests the pattern used by k6-cloud where:
// 1. kustomize.build() generates base manifests
// 2. withImages() applies image overrides and adds hash suffixes to job names
// 3. The hash is computed from std.md5(std.toString(job_object))

local tanka = import 'github.com/grafana/jsonnet-libs/tanka-util/main.libsonnet';
local kz = tanka.kustomize.new(std.thisFile);

// Simulated images config (like waves/dev.jsonnet)
local images = {
  // Deployment images
  deployment_myapp: 'myapp/server:1.0.0',
  // Job images and tags
  job_database_migration: 'myapp/migration:1.0.0',
  job_database_migration_tag: 'abc123',
};

// Build kustomize manifests
local kustomization = kz.build(path='./kustomize');

// Image override logic (simplified from k6_cloud.withImages)
local withImages(images) = {
  local base = super.kustomization,
  
  // Image overrides for deployments
  local imageOverrides = {
    [workloadName]+: {
      local this = self,
      
      getImageName(image)::
        local name = (
          if std.length(std.findSubstr('@', image)) != 0
          then std.split(image, '@')[0]
          else if std.length(std.findSubstr(':', image)) != 0
          then std.split(image, ':')[0]
          else image
        );
        local parts = std.split(name, '/');
        parts[std.length(parts) - 1],

      local targetImage = images[workloadName],
      local targetImageName = this.getImageName(targetImage),

      patchContainer(container)::
        local containerImageName = this.getImageName(container.image);
        container {
          image: if targetImageName == containerImageName then targetImage else super.image,
        },

      spec+: {
        template+: {
          spec+: {
            containers: std.map(this.patchContainer, super.containers),
          },
        },
      },
    }
    for workloadName in std.objectFields(images)
    if std.objectHas(base, workloadName)
  },

  // Job name tag hashing (the key part being tested)
  local jobKey(tag_key) = std.strReplace(tag_key, '_tag', ''),
  local addJobNameTags = {
    local tag = images[key],
    // Hash the whole job object and add to name
    local hashTag = std.md5(std.toString(super[jobKey(key)]))[:10],
    [jobKey(key)]+: {
      metadata+: {
        name: std.join('-', [super.name, tag, hashTag]),
      },
    }
    for key in std.objectFields(images)
    if std.native('regexMatch')('job_.*_tag', key) && std.objectHas(base, jobKey(key))
  },

  kustomization+: imageOverrides + addJobNameTags,
};

// Apply the pattern
local result = {
  kustomization: kustomization,
} + withImages(images);

// Inline environment with the processed manifests
{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'kustomize-job-hash-test',
  },
  spec: {
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'default',
  },
  data: result.kustomization,
}
