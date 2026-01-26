#!/usr/bin/env bash
set -euo pipefail

# Extra arguments to pass to hyperfine
HYPERFINE_ARGS=("$@")

# Test fixtures to benchmark
FIXTURES_DIR="test_fixtures/golden_envs"

cat <<EOF
# RTK vs Tanka Benchmarks

Comparing rtk (Rust implementation) with tk (original Tanka) using test fixtures.

## Versions

- tk: $(tk --version)
- rtk: $(rtk --version)

## Export Benchmarks

EOF

# Function to run a benchmark
run_benchmark() {
  local name="$1"
  local env_path="$2"

  echo "### $name"
  echo ""

  # Create temp directory and scripts
  TEMP_DIR="$(mktemp -d)"
  TK_SCRIPT="$(mktemp)"
  RTK_SCRIPT="$(mktemp)"

  # Single trap to clean up everything on function return
  trap 'rm -f "${TK_SCRIPT}" "${RTK_SCRIPT}"; rm -rf "${TEMP_DIR}"' RETURN

  # Pass PATH to ensure tk can find jrsonnet
  cat >"${TK_SCRIPT}" <<EOF
#!/bin/sh
export PATH="$PATH"
rm -rf "\$2/tk" && cd "\$1" && tk export "\$2/tk" . --format '{{ .metadata.namespace | default "_cluster" }}/{{.kind}}-{{.metadata.name}}' --extension yaml --recursive 2>/dev/null
EOF

  cat >"${RTK_SCRIPT}" <<EOF
#!/bin/sh
export PATH="$PATH"
rm -rf "\$2/rtk" && rtk export "\$2/rtk" "\$1" --format '{{ .metadata.namespace | default "_cluster" }}/{{.kind}}-{{.metadata.name}}' --extension yaml --recursive
EOF

  chmod +x "${TK_SCRIPT}" "${RTK_SCRIPT}"

  hyperfine -N --warmup 5 \
    "${HYPERFINE_ARGS[@]}" \
    -n "tk" "${TK_SCRIPT} ${env_path} ${TEMP_DIR}" \
    -n "rtk" "${RTK_SCRIPT} ${env_path} ${TEMP_DIR}"
  echo ""
}

# Find and benchmark all golden test fixtures
if [ ! -d "${FIXTURES_DIR}" ]; then
  echo "Error: ${FIXTURES_DIR} not found."
  echo "Please run this script from the rustanka repository root directory."
  exit 1
fi

for env_dir in "${FIXTURES_DIR}"/*/; do
  env_name="$(basename "${env_dir}")"
  # Convert to absolute path for tk's cd command
  abs_env_dir="$(cd "${env_dir}" && pwd)"

  # `exportJsonnetImplementation` hardcodes an absolute path to the Jsonnet
  # implementation being run (`/usr/local/bin/jrsonnet`), which is not
  # compatible with how we install `jrsonnet` here. It would need to be able to
  # look it up on the `PATH` instead.
  if grep -qs 'exportJsonnetImplementation' "${env_dir}/spec.json" "${env_dir}/main.jsonnet" 2>/dev/null; then
    echo "Skipping ${env_name} (uses exportJsonnetImplementation, can't currently be tested)" >&2

    continue
  fi

  run_benchmark "${env_name}" "${abs_env_dir}"
done
