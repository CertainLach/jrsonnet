#!/usr/bin/env bash
set -euo pipefail

# Extra arguments to pass to hyperfine
HYPERFINE_ARGS=("$@")

# Test fixtures to benchmark
FIXTURES_DIR="test_fixtures/golden_envs"
DIFF_FIXTURES_DIR="cmds/rtk/tests/testdata/diff"

# Cleanup tracking
CLEANUP_FILES=()
CLEANUP_DIRS=()
CLEANUP_PIDS=()

cleanup() {
  for pid in "${CLEANUP_PIDS[@]}"; do
    kill "${pid}" 2>/dev/null || true
  done

  for file in "${CLEANUP_FILES[@]}"; do
    rm -f "${file}"
  done

  for dir in "${CLEANUP_DIRS[@]}"; do
    rm -rf "${dir}"
  done

  CLEANUP_FILES=()
  CLEANUP_DIRS=()
  CLEANUP_PIDS=()
}

trap cleanup EXIT

# Create a temp directory and register for cleanup
make_temp_dir() {
  local dir

  dir="$(mktemp -d)"
  CLEANUP_DIRS+=("${dir}")

  echo "${dir}"
}

# Create a temp file and register for cleanup
make_temp_file() {
  local file

  file="$(mktemp)"
  CLEANUP_FILES+=("${file}")

  echo "${file}"
}

# Create a wrapper script that sets up PATH
make_wrapper_script() {
  local script

  script="$(make_temp_file)"
  cat >"${script}" <<HEADER
#!/bin/sh
export PATH="${PATH}"
HEADER
  cat >>"${script}"
  chmod +x "${script}"

  echo "${script}"
}

cat <<EOF
# RTK vs Tanka Benchmarks

Comparing rtk (Rust implementation) with tk (original Tanka) using test fixtures.

## Versions

- tk: $(tk --version)
- rtk: $(rtk --version)

## Export Benchmarks

EOF

run_export_benchmark() {
  local name="$1"
  local env_path="$2"

  echo "### ${name}"
  echo ""

  local temp_dir
  temp_dir="$(make_temp_dir)"

  local tk_script
  tk_script="$(
    make_wrapper_script <<SCRIPT
rm -rf "\$2/tk" && cd "\$1" && tk export "\$2/tk" . --format '{{ .metadata.namespace | default "_cluster" }}/{{.kind}}-{{.metadata.name}}' --extension yaml --recursive 2>/dev/null
SCRIPT
  )"

  local rtk_script
  rtk_script="$(
    make_wrapper_script <<SCRIPT
rm -rf "\$2/rtk" && rtk export "\$2/rtk" "\$1" --format '{{ .metadata.namespace | default "_cluster" }}/{{.kind}}-{{.metadata.name}}' --extension yaml --recursive
SCRIPT
  )"

  hyperfine -N --warmup 5 \
    "${HYPERFINE_ARGS[@]}" \
    -n "tk" "${tk_script} ${env_path} ${temp_dir}" \
    -n "rtk" "${rtk_script} ${env_path} ${temp_dir}"
  echo ""

  cleanup
}

# Find and benchmark all golden test fixtures
if [ ! -d "${FIXTURES_DIR}" ]; then
  echo "Error: ${FIXTURES_DIR} not found."
  echo "Please run this script from the rustanka repository root directory."
  exit 1
fi

for env_dir in "${FIXTURES_DIR}"/*/; do
  env_name="$(basename "${env_dir}")"
  abs_env_dir="$(cd "${env_dir}" && pwd)"

  if grep -qs 'exportJsonnetImplementation' "${env_dir}/spec.json" "${env_dir}/main.jsonnet" 2>/dev/null; then
    echo "Skipping ${env_name} (uses exportJsonnetImplementation)" >&2
    continue
  fi

  run_export_benchmark "${env_name}" "${abs_env_dir}"
done

# Diff benchmarks
if [ -d "${DIFF_FIXTURES_DIR}" ]; then
  cat <<EOF

## Diff Benchmarks

EOF

  run_diff_benchmark() {
    local name="$1"
    local test_dir="$2"

    echo "### ${name}"
    echo ""

    local env_dir="${test_dir}/environment"
    local cluster_dir="${test_dir}/cluster"

    if [ ! -d "${cluster_dir}" ]; then
      echo "Skipping (no cluster state)"
      echo ""
      return
    fi

    local temp_dir kubeconfig pid_file
    temp_dir="$(make_temp_dir)"
    kubeconfig="${temp_dir}/kubeconfig.yaml"
    pid_file="${temp_dir}/mock.pid"

    # Start mock server daemon (blocks until ready)
    if ! mock-k8s-server -d "${cluster_dir}" -k "${kubeconfig}" -p "${pid_file}" 2>/dev/null; then
      echo "Error: mock server failed to start"
      return 1
    fi

    CLEANUP_PIDS+=("$(cat "${pid_file}")")

    local tk_script
    tk_script="$(
      make_wrapper_script <<SCRIPT
export KUBECONFIG="${kubeconfig}"
cd "\$1" && tk diff . 2>/dev/null || true
SCRIPT
    )"

    local rtk_script
    rtk_script="$(
      make_wrapper_script <<SCRIPT
export KUBECONFIG="${kubeconfig}"
rtk diff "\$1" || true
SCRIPT
    )"

    hyperfine -N --warmup 3 \
      "${HYPERFINE_ARGS[@]}" \
      -n "tk diff" "${tk_script} ${env_dir}" \
      -n "rtk diff" "${rtk_script} ${env_dir}"
    echo ""

    cleanup
  }

  for test_dir in "${DIFF_FIXTURES_DIR}"/*/; do
    test_name="$(basename "${test_dir}")"

    case "${test_name}" in
    *error* | *invalid*) continue ;;
    esac

    abs_test_dir="$(cd "${test_dir}" && pwd)"
    run_diff_benchmark "${test_name}" "${abs_test_dir}"
  done
fi
