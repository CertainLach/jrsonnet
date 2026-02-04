#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["pyyaml"]
# ///
"""
Benchmark runner that executes benchmarks defined in YAML config files.

Supports two modes:
1. Generated fixtures mode: Uses `fixtures` config to generate test environments
2. Diff mode: Uses `fixtures_dir` to point to pre-existing test fixtures with mock K8s server
"""

import argparse
import json
import os
import signal
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

import yaml


# =============================================================================
# Configuration Classes
# =============================================================================

@dataclass
class GeneratedFixtures:
    """Configuration for dynamically generated fixtures."""
    static_envs: int
    inline_files: int
    envs_per_inline_file: int
    resources_per_env: int

    @property
    def total_envs(self) -> int:
        return self.static_envs + self.inline_files * self.envs_per_inline_file

    @property
    def total_env_libs(self) -> int:
        return self.static_envs + self.inline_files

    @property
    def total_lib_files(self) -> int:
        return 1 + self.total_env_libs


@dataclass
class Test:
    """A single benchmark test case."""
    name: str
    description: str = ""
    command: str = ""  # Empty for diff benchmarks (implicit command)


@dataclass
class BenchmarkConfig:
    """Configuration for a benchmark suite."""
    name: str
    id: str
    description: str
    tests: list[Test]
    mode: Literal["generated", "diff"]
    # Generated fixtures mode
    fixtures: GeneratedFixtures | None = None
    setup: str | None = None
    prepare: str | None = None
    # Diff mode
    fixtures_dir: str | None = None

    @classmethod
    def from_yaml(cls, path: Path, repo_root: Path) -> "BenchmarkConfig":
        with open(path) as f:
            data = yaml.safe_load(f)

        # Detect mode based on config
        if "fixtures_dir" in data:
            mode = "diff"
            fixtures = None
            fixtures_dir = str(repo_root / data["fixtures_dir"])
            tests = [Test(name=t["name"]) for t in data["tests"]]
        else:
            mode = "generated"
            fixtures = GeneratedFixtures(**data["fixtures"])
            fixtures_dir = None
            tests = [Test(**t) for t in data["tests"]]

        return cls(
            name=data["name"],
            id=data["id"],
            description=data["description"],
            tests=tests,
            mode=mode,
            fixtures=fixtures,
            fixtures_dir=fixtures_dir,
            setup=data.get("setup"),
            prepare=data.get("prepare"),
        )


# =============================================================================
# Benchmark Runner
# =============================================================================

class BenchmarkRunner:
    def __init__(
        self,
        config: BenchmarkConfig,
        repo_root: Path,
        hyperfine_args: list[str],
        rtk_path: Path | None = None,
        rtk_base_path: Path | None = None,
    ):
        self.config = config
        self.repo_root = repo_root
        self.hyperfine_args = hyperfine_args
        self.rtk_path = rtk_path
        self.rtk_base_path = rtk_base_path
        self.rtk: Path | None = None
        self.rtk_base: Path | None = None
        self.mock_server: Path | None = None
        self.mock_server_pid: int | None = None
        self.fixtures_dir: Path | None = None
        self.export_dir_tk: Path | None = None
        self.export_dir_rtk: Path | None = None
        self.export_dir_rtk_base: Path | None = None

    # -------------------------------------------------------------------------
    # Setup & Build
    # -------------------------------------------------------------------------

    def check_dependencies(self) -> None:
        """Check that required commands are available."""
        required = ["tk", "hyperfine"]
        # Only need cargo if we're building rtk from source
        # (mock-k8s-server for diff mode may also need cargo, but we check that later)
        if not self.rtk_path:
            required.append("cargo")
        if self.config.mode == "generated":
            required.append("jq")

        for cmd in required:
            result = subprocess.run(["which", cmd], capture_output=True)
            if result.returncode != 0:
                print(
                    f"Error: {cmd} is required but not found in PATH", file=sys.stderr)
                sys.exit(1)

    def build_binaries(self) -> None:
        """Build rtk (and mock-k8s-server for diff mode) in release mode."""
        # Use pre-built rtk if provided
        if self.rtk_path:
            if not self.rtk_path.exists():
                print(
                    f"Error: rtk-binary-path does not exist: {self.rtk_path}", file=sys.stderr)
                sys.exit(1)
            self.rtk = self.rtk_path.resolve()
            version = subprocess.run(
                [str(self.rtk), "--version"],
                capture_output=True,
                text=True,
            ).stdout.strip()
            print(
                f"Using pre-built rtk: {self.rtk} ({version})", file=sys.stderr)

            # Look for mock-k8s-server next to the rtk binary (for diff mode)
            if self.config.mode == "diff":
                mock_server_path = self.rtk.parent / "mock-k8s-server"
                if mock_server_path.exists():
                    self.mock_server = mock_server_path
                    print(
                        f"Using pre-built mock-k8s-server: {self.mock_server}", file=sys.stderr)
                else:
                    print("Building mock-k8s-server in release mode...", file=sys.stderr)
                    subprocess.run(
                        ["cargo", "build", "--release", "-p=mock-k8s-server"],
                        cwd=self.repo_root,
                        check=True,
                    )
                    self.mock_server = self.repo_root / "target" / "release" / "mock-k8s-server"
        else:
            print("Building rtk in release mode...", file=sys.stderr)
            subprocess.run(
                ["cargo", "build", "--release", "-p=rtk"],
                cwd=self.repo_root,
                check=True,
            )
            self.rtk = self.repo_root / "target" / "release" / "rtk"

            # Build mock-k8s-server if needed (diff mode only)
            if self.config.mode == "diff":
                print("Building mock-k8s-server in release mode...", file=sys.stderr)
                subprocess.run(
                    ["cargo", "build", "--release", "-p=mock-k8s-server"],
                    cwd=self.repo_root,
                    check=True,
                )
                self.mock_server = self.repo_root / "target" / "release" / "mock-k8s-server"

    def build_rtk_base(self) -> None:
        """Build rtk from base branch if BENCHMARK_BASE_REF is set, or use --rtk-base-binary-path."""
        if self.rtk_base_path:
            if not self.rtk_base_path.exists():
                print(
                    f"Error: rtk-base-binary-path does not exist: {self.rtk_base_path}", file=sys.stderr)
                sys.exit(1)
            self.rtk_base = self.rtk_base_path.resolve()
            version = subprocess.run(
                [str(self.rtk_base), "--version"],
                capture_output=True,
                text=True,
            ).stdout.strip()
            print(
                f"Using pre-built rtk-base: {self.rtk_base} ({version})", file=sys.stderr)
            return

        base_ref = os.environ.get("BENCHMARK_BASE_REF", "")
        if not base_ref:
            return

        print(
            f"Building rtk from base branch ({base_ref})...", file=sys.stderr)
        worktree_dir = self.repo_root / "target-base-src"

        subprocess.run(
            ["git", "worktree", "remove", "--force", str(worktree_dir)],
            cwd=self.repo_root,
            capture_output=True,
        )
        if worktree_dir.exists():
            shutil.rmtree(worktree_dir)

        subprocess.run(
            ["git", "worktree", "add", "--quiet", "--detach",
                str(worktree_dir), f"origin/{base_ref}"],
            cwd=self.repo_root,
            check=True,
        )

        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(self.repo_root / "target-base")
        subprocess.run(
            ["cargo", "build", "--release", "-p", "rtk"],
            cwd=worktree_dir,
            env=env,
            check=True,
        )

        self.rtk_base = self.repo_root / "target-base" / "release" / "rtk"

        subprocess.run(
            ["git", "worktree", "remove", "--force", str(worktree_dir)],
            cwd=self.repo_root,
            check=True,
        )

        version = subprocess.run(
            [str(self.rtk_base), "--version"],
            capture_output=True,
            text=True,
        ).stdout.strip()
        print(f"Built rtk-base: {version}", file=sys.stderr)

    # -------------------------------------------------------------------------
    # Generated Fixtures Mode
    # -------------------------------------------------------------------------

    def generate_fixtures(self, fixtures_dir: Path) -> None:
        """Generate test fixtures for generated mode."""
        assert self.config.fixtures is not None
        self.fixtures_dir = fixtures_dir

        script = f"""
        set -euo pipefail
        NUM_STATIC_ENVS={self.config.fixtures.static_envs}
        NUM_INLINE_FILES={self.config.fixtures.inline_files}
        ENVS_PER_INLINE_FILE={self.config.fixtures.envs_per_inline_file}
        NUM_RESOURCES_PER_ENV={self.config.fixtures.resources_per_env}
        source "{self.repo_root}/rtk-benchmarks/lib/generate-fixtures.sh"
        generate_fixtures "{fixtures_dir}"
        """
        subprocess.run(["bash", "-c", script], check=True)

    def get_path_vars(self, export_dir: Path | None = None) -> dict[str, str]:
        """Get path variables for command substitution."""
        assert self.fixtures_dir is not None
        assert self.config.fixtures is not None

        all_static_env_paths = " ".join(
            str(self.fixtures_dir / f"static-{i:04d}")
            for i in range(1, self.config.fixtures.static_envs + 1)
        )
        all_inline_file_paths = " ".join(
            str(self.fixtures_dir / f"inline-{i:02d}" / "main.jsonnet")
            for i in range(1, self.config.fixtures.inline_files + 1)
        )
        all_static_main_files = " ".join(
            str(self.fixtures_dir / f"static-{i:04d}" / "main.jsonnet")
            for i in range(1, self.config.fixtures.static_envs + 1)
        )
        all_static_lib_files_rel = [
            f"lib/env-static-{i:04d}/main.libsonnet"
            for i in range(1, self.config.fixtures.static_envs + 1)
        ]
        all_inline_lib_files_rel = [
            f"lib/env-inline-{i:02d}/main.libsonnet"
            for i in range(1, self.config.fixtures.inline_files + 1)
        ]
        all_env_lib_files = " ".join(
            all_static_lib_files_rel + all_inline_lib_files_rel)
        global_lib_file = "lib/global/main.libsonnet"
        all_lib_files = f"{global_lib_file} {all_env_lib_files}"
        all_static_main_files_rel = " ".join(
            f"static-{i:04d}/main.jsonnet"
            for i in range(1, self.config.fixtures.static_envs + 1)
        )
        all_inline_files_rel = " ".join(
            f"inline-{i:02d}/main.jsonnet"
            for i in range(1, self.config.fixtures.inline_files + 1)
        )
        all_jsonnet_files = f"{all_static_main_files_rel} {all_inline_files_rel} {all_lib_files}"

        result = {
            "fixtures_dir": str(self.fixtures_dir),
            "single_static_dir": str(self.fixtures_dir / "static-0001"),
            "single_inline_dir": str(self.fixtures_dir / "inline-01"),
            "single_inline_file": str(self.fixtures_dir / "inline-01" / "main.jsonnet"),
            "single_static_main_file": str(self.fixtures_dir / "static-0001" / "main.jsonnet"),
            "global_lib_file": global_lib_file,
            "single_env_lib_file": "lib/env-static-0001/main.libsonnet",
            "all_static_env_paths": all_static_env_paths,
            "all_inline_file_paths": all_inline_file_paths,
            "all_static_main_files": all_static_main_files,
            "all_env_lib_files": all_env_lib_files,
            "all_lib_files": all_lib_files,
            "all_jsonnet_files": all_jsonnet_files,
            "static_envs": str(self.config.fixtures.static_envs),
            "inline_files": str(self.config.fixtures.inline_files),
            "envs_per_inline_file": str(self.config.fixtures.envs_per_inline_file),
            "resources_per_env": str(self.config.fixtures.resources_per_env),
            "total_env_libs": str(self.config.fixtures.total_env_libs),
            "total_lib_files": str(self.config.fixtures.total_lib_files),
            "total_envs": str(self.config.fixtures.total_envs),
        }
        if export_dir:
            result["export_dir"] = str(export_dir)
        return result

    def expand_command(self, command: str, export_dir: Path | None = None) -> str:
        """Expand placeholders in command."""
        result = command
        for key, value in self.get_path_vars(export_dir).items():
            result = result.replace(f"{{{key}}}", value)
        return result

    def run_command(self, binary: str, command: str) -> subprocess.CompletedProcess:
        """Run a command with the given binary."""
        return subprocess.run(
            ["sh", "-c", f"{binary} {command}"],
            capture_output=True,
            text=True,
            cwd=self.fixtures_dir,
        )

    def _clear_export_dir(self, export_dir: Path) -> None:
        if export_dir.exists():
            shutil.rmtree(export_dir)
            export_dir.mkdir()

    def run_setup(self) -> None:
        """Run the setup command if configured."""
        if not self.config.setup:
            return

        print("Clearing export directories...", file=sys.stderr, flush=True)
        assert self.export_dir_tk and self.export_dir_rtk
        self._clear_export_dir(self.export_dir_tk)
        self._clear_export_dir(self.export_dir_rtk)
        if self.export_dir_rtk_base:
            self._clear_export_dir(self.export_dir_rtk_base)

        for name, binary, export_dir in [
            ("tk", "tk", self.export_dir_tk),
            ("rtk", str(self.rtk), self.export_dir_rtk),
            ("rtk-base", str(self.rtk_base)
             if self.rtk_base else None, self.export_dir_rtk_base),
        ]:
            if binary is None or export_dir is None:
                continue
            command = self.expand_command(self.config.setup, export_dir)
            print(f"Running setup: {name} {command}...",
                  file=sys.stderr, flush=True)
            result = self.run_command(binary, command)
            if result.returncode != 0:
                print(
                    f"ERROR: {name} setup failed with exit code {result.returncode}", file=sys.stderr)
                print(f"stderr: {result.stderr}", file=sys.stderr)
                sys.exit(1)

        print("Setup complete.", file=sys.stderr)

    def validate_test(self, test: Test) -> None:
        """Validate that tk and rtk produce matching output."""
        if self.config.prepare:
            tk_prepare = self.expand_command(
                self.config.prepare, self.export_dir_tk)
            rtk_prepare = self.expand_command(
                self.config.prepare, self.export_dir_rtk)
            subprocess.run(["sh", "-c", tk_prepare],
                           cwd=self.fixtures_dir, check=True)
            subprocess.run(["sh", "-c", rtk_prepare],
                           cwd=self.fixtures_dir, check=True)

        tk_command = self.expand_command(test.command, self.export_dir_tk)
        rtk_command = self.expand_command(test.command, self.export_dir_rtk)
        print(f"Validating {test.name}... ",
              end="", file=sys.stderr, flush=True)

        tk_result = self.run_command("tk", tk_command)
        rtk_result = self.run_command(str(self.rtk), rtk_command)

        if tk_result.returncode != 0:
            print(
                f"ERROR: tk failed with exit code {tk_result.returncode}", file=sys.stderr)
            self._fail_validation(f"tk command failed: {tk_command}")

        if rtk_result.returncode != 0:
            print(
                f"ERROR: rtk failed with exit code {rtk_result.returncode}", file=sys.stderr)
            self._fail_validation(f"rtk command failed: {rtk_command}")

        if test.command.startswith("export "):
            pass
        elif "--json" in test.command or test.command.startswith("eval "):
            if not self._json_equal(tk_result.stdout, rtk_result.stdout):
                print("JSON MISMATCH!", file=sys.stderr)
                self._fail_validation(
                    f"rtk JSON output differs from tk for: {test.command}")
        else:
            if tk_result.stdout != rtk_result.stdout:
                print("OUTPUT MISMATCH!", file=sys.stderr)
                self._fail_validation(
                    f"rtk output differs from tk for: {test.command}")

        print("OK", file=sys.stderr, flush=True)

    def _json_equal(self, json1: str, json2: str) -> bool:
        try:
            return json.loads(json1) == json.loads(json2)
        except json.JSONDecodeError:
            return json1 == json2

    def _fail_validation(self, message: str) -> None:
        print(f"\n## Validation Failed\n\n{message}\n", flush=True)
        sys.exit(1)

    def _check_rtk_base_supports_command(self, test: Test) -> bool:
        if not self.rtk_base:
            return False
        rtk_base_command = self.expand_command(
            test.command, self.export_dir_rtk_base)
        result = self.run_command(str(self.rtk_base), rtk_base_command)
        if result.returncode != 0:
            if "not implemented" in result.stderr.lower() or "not implemented" in result.stdout.lower():
                print(
                    "  (rtk-base does not support this command, skipping)", file=sys.stderr)
                return False
        return True

    def run_generated_benchmark(self, test: Test, output_file: Path, index: int) -> dict:
        """Run benchmark for generated fixtures mode."""
        tk_command = self.expand_command(test.command, self.export_dir_tk)
        rtk_command = self.expand_command(test.command, self.export_dir_rtk)
        description = self.expand_command(test.description)

        print(f"### {test.name}")
        print()
        print(description)
        print()

        include_rtk_base = self.rtk_base and self.export_dir_rtk_base and self._check_rtk_base_supports_command(
            test)

        temp_md = output_file.with_suffix(f".{index}")
        temp_json = output_file.with_suffix(f".{index}.json")
        cd_prefix = f"cd {self.fixtures_dir} && "

        prepare_args = []
        if self.config.prepare:
            tk_prepare = self.expand_command(
                self.config.prepare, self.export_dir_tk)
            rtk_prepare = self.expand_command(
                self.config.prepare, self.export_dir_rtk)
            prepare_args = [
                "--prepare", f"sh -c '{tk_prepare}'", "--prepare", f"sh -c '{rtk_prepare}'"]
            if include_rtk_base:
                rtk_base_prepare = self.expand_command(
                    self.config.prepare, self.export_dir_rtk_base)
                prepare_args.extend(
                    ["--prepare", f"sh -c '{rtk_base_prepare}'"])

        args = [
            "hyperfine", "-N",
            *self.hyperfine_args,
            *prepare_args,
            "--export-markdown", str(temp_md),
            "--export-json", str(temp_json),
            "--warmup", "1",
            "-n", "tk", f"sh -c '{cd_prefix}tk {tk_command} >/dev/null'",
            "-n", "rtk", f"sh -c '{cd_prefix}{self.rtk} {rtk_command} >/dev/null'",
        ]

        if include_rtk_base:
            rtk_base_command = self.expand_command(
                test.command, self.export_dir_rtk_base)
            args.extend(
                ["-n", "rtk-base", f"sh -c '{cd_prefix}{self.rtk_base} {rtk_base_command} >/dev/null'"])

        subprocess.run(args, check=True, stdout=subprocess.DEVNULL)

        with open(temp_md) as f:
            print(f.read())
        print()

        return self._parse_benchmark_json(test.name, temp_json, "tk", "rtk", "rtk-base")

    # -------------------------------------------------------------------------
    # Diff Mode
    # -------------------------------------------------------------------------

    def start_mock_server(self, cluster_dir: Path, kubeconfig_path: Path, pid_file: Path) -> None:
        """Start the mock Kubernetes server."""
        assert self.mock_server is not None
        # The mock server daemonizes by default. When it daemonizes, the parent
        # process exits after writing the ready signal. We must use DEVNULL for
        # stdio to prevent Python from waiting for the daemon's inherited file
        # descriptors to close.
        result = subprocess.run(
            [str(self.mock_server), "-d", str(cluster_dir), "-k",
             str(kubeconfig_path), "-p", str(pid_file)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=10,
        )
        if result.returncode != 0:
            print(
                f"Failed to start mock server (exit code {result.returncode})", file=sys.stderr)
            sys.exit(1)
        self.mock_server_pid = int(pid_file.read_text().strip())

    def stop_mock_server(self) -> None:
        """Stop the mock Kubernetes server."""
        if self.mock_server_pid:
            try:
                os.kill(self.mock_server_pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            self.mock_server_pid = None

    def run_diff_benchmark(self, test: Test, temp_dir: Path, output_file: Path, index: int) -> dict:
        """Run benchmark for diff mode."""
        assert self.config.fixtures_dir is not None
        test_dir = Path(self.config.fixtures_dir) / test.name
        env_dir = test_dir / "environment"
        cluster_dir = test_dir / "cluster"

        print(f"### {test.name}")
        print()

        if not cluster_dir.exists():
            print("_Skipped (no cluster state)_")
            print()
            return {"name": test.name, "skipped": True}

        kubeconfig = temp_dir / f"kubeconfig-{test.name}.yaml"
        pid_file = temp_dir / f"mock-{test.name}.pid"

        self.start_mock_server(cluster_dir, kubeconfig, pid_file)

        try:
            temp_md = output_file.with_suffix(f".{index}")
            temp_json = output_file.with_suffix(f".{index}.json")

            tk_cmd = f"cd {env_dir} && KUBECONFIG={kubeconfig} tk diff . </dev/null 2>/dev/null || true"
            rtk_cmd = f"KUBECONFIG={kubeconfig} {self.rtk} diff {env_dir} </dev/null 2>/dev/null || true"

            args = [
                "hyperfine", "-N",
                *self.hyperfine_args,
                "--export-markdown", str(temp_md),
                "--export-json", str(temp_json),
                "-n", "tk diff", f"sh -c '{tk_cmd}'",
                "-n", "rtk diff", f"sh -c '{rtk_cmd}'",
            ]

            if self.rtk_base:
                rtk_base_cmd = f"KUBECONFIG={kubeconfig} {self.rtk_base} diff {env_dir} </dev/null 2>/dev/null || true"
                check_result = subprocess.run(
                    ["sh", "-c", rtk_base_cmd], capture_output=True, text=True, timeout=30)
                if "not implemented" not in check_result.stderr.lower():
                    args.extend(
                        ["-n", "rtk-base diff", f"sh -c '{rtk_base_cmd}'"])

            subprocess.run(args, check=True, stdout=subprocess.DEVNULL)

            with open(temp_md) as f:
                print(f.read())
            print()

            return self._parse_benchmark_json(test.name, temp_json, "tk diff", "rtk diff", "rtk-base diff")

        finally:
            self.stop_mock_server()

    # -------------------------------------------------------------------------
    # Common
    # -------------------------------------------------------------------------

    def _parse_benchmark_json(self, test_name: str, json_path: Path, tk_name: str, rtk_name: str, rtk_base_name: str) -> dict:
        """Parse hyperfine JSON output."""
        with open(json_path) as f:
            data = json.load(f)

        results = {}
        for result in data["results"]:
            results[result["command"]] = {
                "mean": result["mean"], "stddev": result["stddev"]}

        summary = {"name": test_name}

        if tk_name in results and rtk_name in results:
            tk_mean = results[tk_name]["mean"]
            rtk_mean = results[rtk_name]["mean"]
            summary["vs_tk"] = round(tk_mean / rtk_mean, 2)
            summary["rtk_mean"] = rtk_mean
            summary["rtk_stddev"] = results[rtk_name]["stddev"]

        if rtk_base_name in results and rtk_name in results:
            base_mean = results[rtk_base_name]["mean"]
            base_stddev = results[rtk_base_name]["stddev"]
            rtk_mean = results[rtk_name]["mean"]
            rtk_stddev = results[rtk_name]["stddev"]
            combined_stddev = (rtk_stddev**2 + base_stddev**2) ** 0.5
            diff = abs(rtk_mean - base_mean)
            if diff <= combined_stddev:
                summary["vs_base"] = "equal"
            elif rtk_mean < base_mean:
                summary["vs_base"] = f"{round(base_mean / rtk_mean, 2)}x faster"
            else:
                summary["vs_base"] = f"{round(rtk_mean / base_mean, 2)}x slower"

        return summary

    def print_header(self) -> None:
        """Print benchmark header."""
        print("<details>", flush=True)
        print("<summary>Test Configuration & Versions</summary>", flush=True)
        print(flush=True)
        print(f"**{self.config.name}**: {self.config.description}", flush=True)
        print(flush=True)
        print("### Test Configuration", flush=True)
        print(flush=True)
        if self.config.mode == "generated" and self.config.fixtures:
            print(
                f"- Static environments: {self.config.fixtures.static_envs}", flush=True)
            print(f"- Inline environment files: {self.config.fixtures.inline_files} "
                  f"({self.config.fixtures.envs_per_inline_file} envs each = "
                  f"{self.config.fixtures.inline_files * self.config.fixtures.envs_per_inline_file} total)", flush=True)
            print(
                f"- Resources per environment: {self.config.fixtures.resources_per_env}", flush=True)
            print(f"- Lib files: {self.config.fixtures.total_lib_files} "
                  f"(1 global + {self.config.fixtures.total_env_libs} env-specific)", flush=True)
            print(
                f"- Total environments: {self.config.fixtures.total_envs}", flush=True)
        else:
            print(
                f"- Fixtures directory: `{self.config.fixtures_dir}`", flush=True)
            print(f"- Test cases: {len(self.config.tests)}", flush=True)
        print(flush=True)

    def print_versions(self) -> None:
        """Print version information."""
        tk_result = subprocess.run(
            ["tk", "--version"], capture_output=True, text=True)
        tk_version = (tk_result.stdout or tk_result.stderr).strip()
        rtk_version = subprocess.run(
            [str(self.rtk), "--version"], capture_output=True, text=True).stdout.strip()

        print("### Versions", flush=True)
        print(flush=True)
        print(f"- tk: {tk_version}", flush=True)
        print(f"- rtk: {rtk_version}", flush=True)
        if self.rtk_base:
            rtk_base_version = subprocess.run(
                [str(self.rtk_base), "--version"], capture_output=True, text=True).stdout.strip()
            print(f"- rtk-base: {rtk_base_version}", flush=True)
        print(flush=True)
        print("</details>", flush=True)
        print(flush=True)

    def run(self) -> None:
        """Run the benchmark."""
        self.check_dependencies()
        self.build_binaries()
        self.build_rtk_base()

        self.print_header()
        self.print_versions()

        output_file = Path(os.environ.get(
            "BENCHMARK_MARKDOWN_OUTPUT", tempfile.mktemp()))

        summaries = []

        if self.config.mode == "generated":
            with tempfile.TemporaryDirectory() as tmpdir:
                self.generate_fixtures(Path(tmpdir))

                self.export_dir_tk = Path(tmpdir) / "export-output-tk"
                self.export_dir_tk.mkdir(exist_ok=True)
                self.export_dir_rtk = Path(tmpdir) / "export-output-rtk"
                self.export_dir_rtk.mkdir(exist_ok=True)
                if self.rtk_base:
                    self.export_dir_rtk_base = Path(
                        tmpdir) / "export-output-rtk-base"
                    self.export_dir_rtk_base.mkdir(exist_ok=True)

                self.run_setup()

                print("Validating outputs match before benchmarking...",
                      file=sys.stderr)
                for test in self.config.tests:
                    self.validate_test(test)
                print(file=sys.stderr)

                print("## Benchmarks")
                print()

                for i, test in enumerate(self.config.tests, 1):
                    summary = self.run_generated_benchmark(
                        test, output_file, i)
                    summaries.append(summary)
        else:
            print("## Benchmarks", flush=True)
            print(flush=True)

            with tempfile.TemporaryDirectory() as temp_dir:
                for i, test in enumerate(self.config.tests, 1):
                    summary = self.run_diff_benchmark(
                        test, Path(temp_dir), output_file, i)
                    summaries.append(summary)

        # Write summary JSON
        summary_json_path = Path(os.environ.get(
            "BENCHMARK_SUMMARY_OUTPUT", "benchmark-summary.json"))
        output = {
            "benchmark_name": self.config.name,
            "benchmark_id": self.config.id,
            "tests": summaries,
        }
        with open(summary_json_path, "w") as f:
            json.dump(output, f, indent=2)

        print(f"Markdown output written to: {output_file}", file=sys.stderr)
        print(f"Summary JSON written to: {summary_json_path}", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(
        description="Run benchmarks from YAML config",
        usage="%(prog)s config [--rtk-binary-path PATH] [--rtk-base-binary-path PATH] [-- hyperfine_args...]",
    )
    parser.add_argument("config", type=Path,
                        help="Path to benchmark YAML config file")
    parser.add_argument("--rtk-binary-path", type=Path,
                        help="Path to pre-built rtk binary (skips building from current branch)")
    parser.add_argument("--rtk-base-binary-path", type=Path,
                        help="Path to pre-built rtk binary for baseline comparison")

    args, hyperfine_args = parser.parse_known_args()

    if hyperfine_args and hyperfine_args[0] == "--":
        hyperfine_args = hyperfine_args[1:]

    repo_root = Path(__file__).parent.parent.resolve()
    config = BenchmarkConfig.from_yaml(args.config, repo_root)
    runner = BenchmarkRunner(
        config, repo_root, hyperfine_args,
        rtk_path=args.rtk_binary_path,
        rtk_base_path=args.rtk_base_binary_path,
    )
    runner.run()


if __name__ == "__main__":
    main()
