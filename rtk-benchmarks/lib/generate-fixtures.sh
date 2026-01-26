#!/usr/bin/env bash
# Common fixture generation for benchmarks
# Source this file from benchmark scripts after setting configuration variables

set -euo pipefail

# Default configuration (can be overridden before sourcing)
: "${NUM_STATIC_ENVS:=100}"
: "${NUM_INLINE_FILES:=9}"
: "${ENVS_PER_INLINE_FILE:=100}"
: "${NUM_RESOURCES_PER_ENV:=20}"

# Generate all fixtures in FIXTURES_DIR
generate_fixtures() {
  local fixtures_dir="$1"
  
  echo "Generating test fixtures in ${fixtures_dir}..." >&2

  # Create jsonnetfile.json at root (required by tk to identify project root)
  cat >"${fixtures_dir}/jsonnetfile.json" <<'EOF'
{
  "version": 1,
  "dependencies": [],
  "legacyImports": true
}
EOF

  # Create lib directory (JPATH root for imports)
  local lib_dir="${fixtures_dir}/lib"
  mkdir -p "${lib_dir}"

  # Generate global lib (lib/global/main.libsonnet -> imported as "global/main.libsonnet")
  echo "Generating global lib..." >&2
  mkdir -p "${lib_dir}/global"
  cat >"${lib_dir}/global/main.libsonnet" <<'JSONNET'
// Global utility library used by all environments
local lib = {
  // Helper function to create a standard label set
  labels(name, component):: {
    'app.kubernetes.io/name': name,
    'app.kubernetes.io/component': component,
    'app.kubernetes.io/managed-by': 'tanka',
  },

  // Helper to create a ConfigMap
  configMap(name, namespace, data):: {
    apiVersion: 'v1',
    kind: 'ConfigMap',
    metadata: {
      name: name,
      namespace: namespace,
      labels: lib.labels(name, 'config'),
    },
    data: data,
  },

  // Helper to create a Deployment
  deployment(name, namespace, image, replicas=1):: {
    apiVersion: 'apps/v1',
    kind: 'Deployment',
    metadata: {
      name: name,
      namespace: namespace,
      labels: lib.labels(name, 'app'),
    },
    spec: {
      replicas: replicas,
      selector: { matchLabels: { app: name } },
      template: {
        metadata: { labels: { app: name } },
        spec: {
          containers: [{
            name: name,
            image: image,
          }],
        },
      },
    },
  },

  // Helper to create a Service
  service(name, namespace, port, targetPort):: {
    apiVersion: 'v1',
    kind: 'Service',
    metadata: {
      name: name,
      namespace: namespace,
      labels: lib.labels(name, 'service'),
    },
    spec: {
      selector: { app: name },
      ports: [{ port: port, targetPort: targetPort }],
    },
  },

  // Global hash for testing
  globalHash:: std.md5('global-lib'),
};

lib
JSONNET

  # Generate static environments with env-specific libs
  echo "Generating ${NUM_STATIC_ENVS} static environments with env-specific libs..." >&2
  for i in $(seq 1 ${NUM_STATIC_ENVS}); do
    padded=$(printf "%04d" "$i")
    env_dir="${fixtures_dir}/static-${padded}"
    mkdir -p "${env_dir}"

    # Create env-specific lib directory (lib/env-static-XXXX/main.libsonnet)
    mkdir -p "${lib_dir}/env-static-${padded}"
    cat >"${lib_dir}/env-static-${padded}/main.libsonnet" <<JSONNET
// Environment-specific library for static-${padded}
{
  envName:: 'static-env-${i}',
  envIndex:: ${i},
  namespace:: 'ns-static-${i}',
  clusterUrl:: 'https://cluster-${i}.example.com',
  
  // Env-specific hash
  envHash:: std.md5('static-${i}'),
  
  // Env-specific configuration
  config:: {
    replicas: 1 + (${i} % 5),
    port: 8080 + ${i},
    metricsPort: 9090 + ${i},
  },
}
JSONNET

    # Create spec.json
    cat >"${env_dir}/spec.json" <<EOF
{
  "apiVersion": "tanka.dev/v1alpha1",
  "kind": "Environment",
  "metadata": {
    "name": "static-env-${i}",
    "labels": {
      "type": "static",
      "index": "${i}"
    }
  },
  "spec": {
    "apiServer": "https://cluster-${i}.example.com",
    "namespace": "ns-static-${i}"
  }
}
EOF

    # Create main.jsonnet that imports both global and env-specific libs via JPATH
    cat >"${env_dir}/main.jsonnet" <<JSONNET
// Static environment ${i}
local global = import 'global/main.libsonnet';
local env = import 'env-static-${padded}/main.libsonnet';

// Use both libs to generate resources
{
  ['cm-%d' % idx]: global.configMap(
    '%s-cm-%d' % [env.envName, idx],
    env.namespace,
    {
      'config.yaml': std.manifestYamlDoc({
        server: {
          port: env.config.port + idx,
          host: '0.0.0.0',
          name: '%s-server-%d' % [env.envName, idx],
        },
        logging: {
          level: if idx % 2 == 0 then 'info' else 'debug',
          format: 'json',
        },
      }),
      envHash: env.envHash,
      globalHash: global.globalHash,
    }
  )
  for idx in std.range(0, ${NUM_RESOURCES_PER_ENV} - 1)
} + {
  ['deploy-%d' % idx]: global.deployment(
    '%s-deploy-%d' % [env.envName, idx],
    env.namespace,
    'nginx:1.%d' % (20 + idx % 10),
    env.config.replicas
  )
  for idx in std.range(0, ${NUM_RESOURCES_PER_ENV} - 1)
} + {
  ['svc-%d' % idx]: global.service(
    '%s-svc-%d' % [env.envName, idx],
    env.namespace,
    80,
    env.config.port + idx
  )
  for idx in std.range(0, ${NUM_RESOURCES_PER_ENV} - 1)
}
JSONNET
  done

  # Generate inline environments with env-specific libs
  echo "Generating ${NUM_INLINE_FILES} inline environment files with env-specific libs (${ENVS_PER_INLINE_FILE} envs each)..." >&2
  for i in $(seq 1 ${NUM_INLINE_FILES}); do
    padded=$(printf "%02d" "$i")
    env_dir="${fixtures_dir}/inline-${padded}"
    mkdir -p "${env_dir}"

    # Create env-specific lib directory (lib/env-inline-XX/main.libsonnet)
    mkdir -p "${lib_dir}/env-inline-${padded}"
    cat >"${lib_dir}/env-inline-${padded}/main.libsonnet" <<JSONNET
// Environment-specific library for inline-${padded}
{
  groupName:: 'inline-group-${i}',
  groupIndex:: ${i},
  
  // Group-specific hash
  groupHash:: std.md5('inline-${i}'),
  
  // Generate env-specific config for a given env index
  envConfig(envIdx):: {
    envName: 'inline-${i}-env-%03d' % envIdx,
    namespace: 'ns-inline-${i}-%03d' % envIdx,
    clusterUrl: 'https://cluster-${i}-%03d.example.com' % envIdx,
    replicas: 1 + (envIdx % 5),
    port: 8080 + envIdx,
  },
}
JSONNET

    # Create main.jsonnet with multiple inline environments using both libs via JPATH
    cat >"${env_dir}/main.jsonnet" <<JSONNET
// Inline environments file ${i}
local global = import 'global/main.libsonnet';
local env = import 'env-inline-${padded}/main.libsonnet';

local makeResources(envIdx) = 
  local cfg = env.envConfig(envIdx);
  {
    ['cm-%d' % idx]: global.configMap(
      '%s-cm-%d' % [cfg.envName, idx],
      cfg.namespace,
      {
        'config.yaml': std.manifestYamlDoc({
          server: {
            port: cfg.port + idx,
            host: '0.0.0.0',
          },
        }),
        groupHash: env.groupHash,
        globalHash: global.globalHash,
      }
    )
    for idx in std.range(0, ${NUM_RESOURCES_PER_ENV} - 1)
  } + {
    ['deploy-%d' % idx]: global.deployment(
      '%s-deploy-%d' % [cfg.envName, idx],
      cfg.namespace,
      'nginx:1.%d' % (20 + idx % 10),
      cfg.replicas
    )
    for idx in std.range(0, ${NUM_RESOURCES_PER_ENV} - 1)
  } + {
    ['svc-%d' % idx]: global.service(
      '%s-svc-%d' % [cfg.envName, idx],
      cfg.namespace,
      80,
      cfg.port + idx
    )
    for idx in std.range(0, ${NUM_RESOURCES_PER_ENV} - 1)
  };

{
  ['env-%03d' % j]: {
    apiVersion: 'tanka.dev/v1alpha1',
    kind: 'Environment',
    metadata: {
      name: env.envConfig(j).envName,
      labels: {
        type: 'inline',
        group: env.groupName,
        index: '%03d' % j,
      },
    },
    spec: {
      apiServer: env.envConfig(j).clusterUrl,
      namespace: env.envConfig(j).namespace,
    },
    data: makeResources(j),
  }
  for j in std.range(0, ${ENVS_PER_INLINE_FILE} - 1)
}
JSONNET
  done

  echo "Fixture generation complete." >&2
  echo "" >&2
}
