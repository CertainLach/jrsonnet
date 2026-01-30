#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["pyyaml"]
# ///
"""
Benchmark runner that executes benchmarks defined in YAML config files.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

import yaml


@dataclass
class Fixtures:
    static_envs: int
    inline_files: int
    envs_per_inline_file: int
    resources_per_env: int

    @property
    def total_envs(self) -> int:
        return self.static_envs + self.inline_files * self.envs_per_inline_file

    @property
    def total_env_libs(self) -> int:
        """Number of env-specific lib files (one per static env + one per inline file)."""
        return self.static_envs + self.inline_files

    @property
    def total_lib_files(self) -> int:
        """Total lib files including global lib."""
        return 1 + self.total_env_libs


@dataclass
class Test:
    name: str
    description: str
    warmup: int
    command: str


@dataclass
class BenchmarkConfig:
    name: str
    id: str
    description: str
    fixtures: Fixtures
    tests: list[Test]
    setup: str | None = None  # Optional setup command run once before benchmarks
    prepare: str | None = None  # Optional prepare command run before each benchmark iteration

    @classmethod
    def from_yaml(cls, path: Path) -> "BenchmarkConfig":
        with open(path) as f:
            data = yaml.safe_load(f)

        fixtures = Fixtures(**data["fixtures"])
        tests = [Test(**t) for t in data["tests"]]

        return cls(
            name=data["name"],
            id=data["id"],
            description=data["description"],
            fixtures=fixtures,
            tests=tests,
            setup=data.get("setup"),
            prepare=data.get("prepare"),
        )


class BenchmarkRunner:
    def __init__(self, config: BenchmarkConfig, repo_root: Path, hyperfine_args: list[str], rtk_base_path: Path | None = None):
        self.config = config
        self.repo_root = repo_root
        self.hyperfine_args = hyperfine_args
        self.rtk_base_path = rtk_base_path  # Pre-built rtk binary for baseline comparison
        self.rtk: Path | None = None
        self.rtk_base: Path | None = None
        self.fixtures_dir: Path | None = None
        self.export_dir_tk: Path | None = None
        self.export_dir_rtk: Path | None = None
        self.export_dir_rtk_base: Path | None = None

    def check_dependencies(self) -> None:
        """Check that required commands are available."""
        for cmd in ["tk", "hyperfine", "jq", "cargo"]:
            result = subprocess.run(["which", cmd], capture_output=True)
            if result.returncode != 0:
                print(f"Error: {cmd} is required but not found in PATH", file=sys.stderr)
                sys.exit(1)

    def build_rtk(self) -> None:
        """Build rtk in release mode."""
        print("Building rtk in release mode...", file=sys.stderr)
        subprocess.run(
            ["cargo", "build", "--release", "-p", "rtk"],
            cwd=self.repo_root,
            check=True,
        )
        self.rtk = self.repo_root / "target" / "release" / "rtk"

    def build_rtk_base(self) -> None:
        """Build rtk from base branch if BENCHMARK_BASE_REF is set, or use --rtk-base-path if provided."""
        # Use pre-built binary if provided via CLI
        if self.rtk_base_path:
            if not self.rtk_base_path.exists():
                print(f"Error: rtk-base-path does not exist: {self.rtk_base_path}", file=sys.stderr)
                sys.exit(1)
            self.rtk_base = self.rtk_base_path.resolve()  # Make absolute
            version = subprocess.run(
                [str(self.rtk_base), "--version"],
                capture_output=True,
                text=True,
            ).stdout.strip()
            print(f"Using pre-built rtk-base: {self.rtk_base} ({version})", file=sys.stderr)
            return

        base_ref = os.environ.get("BENCHMARK_BASE_REF", "")
        if not base_ref:
            self.rtk_base = None
            return

        print(f"Building rtk from base branch ({base_ref})...", file=sys.stderr)

        # Use git worktree to create a separate working directory
        # This avoids checkout conflicts and doesn't require network access
        worktree_dir = self.repo_root / "target-base-src"

        # Remove existing worktree if present
        subprocess.run(
            ["git", "worktree", "remove", "--force", str(worktree_dir)],
            cwd=self.repo_root,
            capture_output=True,  # Ignore errors if worktree doesn't exist
        )
        if worktree_dir.exists():
            shutil.rmtree(worktree_dir)

        # Create worktree at the base branch
        subprocess.run(
            ["git", "worktree", "add", "--quiet", "--detach", str(worktree_dir), f"origin/{base_ref}"],
            cwd=self.repo_root,
            check=True,
        )

        # Build to separate target directory (in main repo, not worktree)
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(self.repo_root / "target-base")
        subprocess.run(
            ["cargo", "build", "--release", "-p", "rtk"],
            cwd=worktree_dir,
            env=env,
            check=True,
        )

        self.rtk_base = self.repo_root / "target-base" / "release" / "rtk"

        # Clean up worktree
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

    def generate_fixtures(self, fixtures_dir: Path) -> None:
        """Generate test fixtures."""
        self.fixtures_dir = fixtures_dir

        # Source the bash library and call generate_fixtures
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

        lib_dir = self.fixtures_dir / "lib"

        # Generate all static env paths
        all_static_env_paths = " ".join(
            str(self.fixtures_dir / f"static-{i:04d}")
            for i in range(1, self.config.fixtures.static_envs + 1)
        )

        # Generate all inline file paths
        all_inline_file_paths = " ".join(
            str(self.fixtures_dir / f"inline-{i:02d}" / "main.jsonnet")
            for i in range(1, self.config.fixtures.inline_files + 1)
        )

        # Generate all static env main.jsonnet paths (absolute for export commands)
        all_static_main_files = " ".join(
            str(self.fixtures_dir / f"static-{i:04d}" / "main.jsonnet")
            for i in range(1, self.config.fixtures.static_envs + 1)
        )

        # Generate all env-specific lib file paths (lib/env-*/main.libsonnet)
        # Use relative paths for tool importers (which runs from fixtures_dir)
        all_static_lib_files_rel = [
            f"lib/env-static-{i:04d}/main.libsonnet"
            for i in range(1, self.config.fixtures.static_envs + 1)
        ]
        all_inline_lib_files_rel = [
            f"lib/env-inline-{i:02d}/main.libsonnet"
            for i in range(1, self.config.fixtures.inline_files + 1)
        ]
        all_env_lib_files = " ".join(all_static_lib_files_rel + all_inline_lib_files_rel)

        # Global lib + all env-specific libs (relative paths)
        global_lib_file = "lib/global/main.libsonnet"
        all_lib_files = f"{global_lib_file} {all_env_lib_files}"

        # All jsonnet files for tool importers (relative paths, runs from fixtures_dir)
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
        full_cmd = f"{binary} {command}"
        return subprocess.run(
            ["sh", "-c", full_cmd],
            capture_output=True,
            text=True,
            cwd=self.fixtures_dir,
        )

    def _clear_export_dir(self, export_dir: Path) -> None:
        """Clear an export directory."""
        if export_dir.exists():
            shutil.rmtree(export_dir)
            export_dir.mkdir()

    def run_setup(self) -> None:
        """Run the setup command if configured."""
        if not self.config.setup:
            return

        # Clear export directories before setup
        print("Clearing export directories...", file=sys.stderr, flush=True)
        assert self.export_dir_tk is not None
        assert self.export_dir_rtk is not None
        self._clear_export_dir(self.export_dir_tk)
        self._clear_export_dir(self.export_dir_rtk)
        if self.export_dir_rtk_base:
            self._clear_export_dir(self.export_dir_rtk_base)

        # Run setup with tk
        tk_command = self.expand_command(self.config.setup, self.export_dir_tk)
        print(f"Running setup: tk {tk_command}...", file=sys.stderr, flush=True)
        tk_result = self.run_command("tk", tk_command)
        if tk_result.returncode != 0:
            print(f"ERROR: tk setup failed with exit code {tk_result.returncode}", file=sys.stderr)
            print(f"stderr: {tk_result.stderr}", file=sys.stderr)
            sys.exit(1)

        # Run setup with rtk
        rtk_command = self.expand_command(self.config.setup, self.export_dir_rtk)
        print(f"Running setup: rtk {rtk_command}...", file=sys.stderr, flush=True)
        rtk_result = self.run_command(str(self.rtk), rtk_command)
        if rtk_result.returncode != 0:
            print(f"ERROR: rtk setup failed with exit code {rtk_result.returncode}", file=sys.stderr)
            print(f"stderr: {rtk_result.stderr}", file=sys.stderr)
            sys.exit(1)

        # Run setup with rtk-base if available
        if self.rtk_base and self.export_dir_rtk_base:
            rtk_base_command = self.expand_command(self.config.setup, self.export_dir_rtk_base)
            print(f"Running setup: rtk-base {rtk_base_command}...", file=sys.stderr, flush=True)
            rtk_base_result = self.run_command(str(self.rtk_base), rtk_base_command)
            if rtk_base_result.returncode != 0:
                print(f"ERROR: rtk-base setup failed with exit code {rtk_base_result.returncode}", file=sys.stderr)
                print(f"stderr: {rtk_base_result.stderr}", file=sys.stderr)
                sys.exit(1)

        print("Setup complete.", file=sys.stderr)

    def validate_test(self, test: Test) -> None:
        """Validate that tk and rtk produce matching output."""
        # Run prepare command before validation if configured (e.g., clear export dir)
        if self.config.prepare:
            tk_prepare = self.expand_command(self.config.prepare, self.export_dir_tk)
            rtk_prepare = self.expand_command(self.config.prepare, self.export_dir_rtk)
            subprocess.run(["sh", "-c", tk_prepare], cwd=self.fixtures_dir, check=True)
            subprocess.run(["sh", "-c", rtk_prepare], cwd=self.fixtures_dir, check=True)

        tk_command = self.expand_command(test.command, self.export_dir_tk)
        rtk_command = self.expand_command(test.command, self.export_dir_rtk)
        print(f"Validating {test.name}... ", end="", file=sys.stderr, flush=True)

        tk_result = self.run_command("tk", tk_command)
        rtk_result = self.run_command(str(self.rtk), rtk_command)

        if tk_result.returncode != 0:
            print(f"ERROR: tk failed with exit code {tk_result.returncode}", file=sys.stderr)
            print(f"stderr: {tk_result.stderr}", file=sys.stderr)
            self._fail_validation(f"tk command failed: {tk_command}")

        if rtk_result.returncode != 0:
            print(f"ERROR: rtk failed with exit code {rtk_result.returncode}", file=sys.stderr)
            print(f"stderr: {rtk_result.stderr}", file=sys.stderr)
            self._fail_validation(f"rtk command failed: {rtk_command}")

        # For JSON output, compare parsed JSON for equality (order-independent)
        # For export commands, output goes to files so skip stdout comparison
        # Otherwise compare byte-for-byte
        if test.command.startswith("export "):
            # Export commands write to files, stdout is just status output
            pass
        elif "--json" in test.command or test.command.startswith("eval "):
            if not self._json_equal(tk_result.stdout, rtk_result.stdout):
                print("JSON MISMATCH!", file=sys.stderr)
                self._show_diff("tk", "rtk", tk_result.stdout, rtk_result.stdout)
                self._fail_validation(f"rtk JSON output differs from tk for: {test.command}")
        else:
            if tk_result.stdout != rtk_result.stdout:
                print("OUTPUT MISMATCH!", file=sys.stderr)
                self._show_diff("tk", "rtk", tk_result.stdout, rtk_result.stdout)
                self._fail_validation(f"rtk output differs from tk for: {test.command}")

        print("OK", file=sys.stderr, flush=True)

    def _json_equal(self, json1: str, json2: str) -> bool:
        """Compare two JSON strings for equality (ignoring key order)."""
        import json
        try:
            return json.loads(json1) == json.loads(json2)
        except json.JSONDecodeError:
            # If not valid JSON, fall back to string comparison
            return json1 == json2

    def _show_diff(self, name1: str, name2: str, output1: str, output2: str) -> None:
        """Show a summary of differences between two outputs."""
        lines1 = output1.splitlines()
        lines2 = output2.splitlines()

        print(f"\n--- {name1} ({len(lines1)} lines, {len(output1)} bytes)", file=sys.stderr)
        print(f"+++ {name2} ({len(lines2)} lines, {len(output2)} bytes)", file=sys.stderr)

        # Show first difference
        for i, (l1, l2) in enumerate(zip(lines1, lines2)):
            if l1 != l2:
                print(f"\nFirst difference at line {i + 1}:", file=sys.stderr)
                print(f"  {name1}: {l1[:200]!r}", file=sys.stderr)
                print(f"  {name2}: {l2[:200]!r}", file=sys.stderr)
                break
        else:
            if len(lines1) != len(lines2):
                print(f"\nLine count differs: {len(lines1)} vs {len(lines2)}", file=sys.stderr)

        sys.stderr.flush()

    def _fail_validation(self, message: str) -> None:
        """Print validation failure and exit."""
        print(f"\n## Validation Failed\n\n{message}\n", flush=True)
        sys.stdout.flush()
        sys.stderr.flush()
        sys.exit(1)

    def run_benchmark(self, test: Test, output_file: Path, index: int) -> dict:
        """Run hyperfine benchmark for a test. Returns summary dict."""
        tk_command = self.expand_command(test.command, self.export_dir_tk)
        rtk_command = self.expand_command(test.command, self.export_dir_rtk)
        description = self.expand_command(test.description)

        print(f"### {test.name}")
        print()
        print(description)
        print()

        # Build hyperfine command - commands need to run from fixtures_dir
        temp_md = output_file.with_suffix(f".{index}")
        temp_json = output_file.with_suffix(f".{index}.json")
        cd_prefix = f"cd {self.fixtures_dir} && "

        # Build prepare commands if configured (each tool needs its own prepare)
        # Wrap in sh -c so shell operators like && work
        prepare_args = []
        if self.config.prepare:
            tk_prepare = self.expand_command(self.config.prepare, self.export_dir_tk)
            rtk_prepare = self.expand_command(self.config.prepare, self.export_dir_rtk)
            prepare_args = ["--prepare", f"sh -c '{tk_prepare}'", "--prepare", f"sh -c '{rtk_prepare}'"]
            if self.rtk_base and self.export_dir_rtk_base:
                rtk_base_prepare = self.expand_command(self.config.prepare, self.export_dir_rtk_base)
                prepare_args.extend(["--prepare", f"sh -c '{rtk_base_prepare}'"])

        args = [
            "hyperfine",
            "-N",
            "--warmup", str(test.warmup),
            *self.hyperfine_args,
            *prepare_args,
            "--export-markdown", str(temp_md),
            "--export-json", str(temp_json),
            "-n", "tk", f"sh -c '{cd_prefix}tk {tk_command} >/dev/null'",
            "-n", "rtk", f"sh -c '{cd_prefix}{self.rtk} {rtk_command} >/dev/null'",
        ]

        if self.rtk_base and self.export_dir_rtk_base:
            rtk_base_command = self.expand_command(test.command, self.export_dir_rtk_base)
            args.extend(["-n", "rtk-base", f"sh -c '{cd_prefix}{self.rtk_base} {rtk_base_command} >/dev/null'"])

        # Capture stdout to hide hyperfine's progress output from markdown
        subprocess.run(args, check=True, stdout=subprocess.DEVNULL)

        # Append markdown table to output
        with open(temp_md) as f:
            print(f.read())
        print()

        # Parse JSON and return summary
        return self._parse_benchmark_json(test.name, temp_json)

    def _parse_benchmark_json(self, test_name: str, json_path: Path) -> dict:
        """Parse hyperfine JSON output and generate summary."""
        import json
        with open(json_path) as f:
            data = json.load(f)

        results = {}
        for result in data["results"]:
            name = result["command"]
            # hyperfine uses the -n name as the command field
            results[name] = {
                "mean": result["mean"],
                "stddev": result["stddev"],
            }

        summary = {"name": test_name}

        # Calculate comparisons
        if "tk" in results and "rtk" in results:
            tk_mean = results["tk"]["mean"]
            rtk_mean = results["rtk"]["mean"]
            rtk_stddev = results["rtk"]["stddev"]
            speedup = tk_mean / rtk_mean
            summary["vs_tk"] = round(speedup, 2)
            summary["rtk_mean"] = rtk_mean
            summary["rtk_stddev"] = rtk_stddev

        if "rtk-base" in results and "rtk" in results:
            base_mean = results["rtk-base"]["mean"]
            base_stddev = results["rtk-base"]["stddev"]
            rtk_mean = results["rtk"]["mean"]
            rtk_stddev = results["rtk"]["stddev"]
            # Check if within 1 stddev (combined)
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
        print("<details>")
        print("<summary>Test Configuration & Versions</summary>")
        print()
        print(f"**{self.config.name}**: {self.config.description}")
        print()
        print("### Test Configuration")
        print()
        print(f"- Static environments: {self.config.fixtures.static_envs}")
        print(f"- Inline environment files: {self.config.fixtures.inline_files} "
              f"({self.config.fixtures.envs_per_inline_file} envs each = "
              f"{self.config.fixtures.inline_files * self.config.fixtures.envs_per_inline_file} total)")
        print(f"- Resources per environment: {self.config.fixtures.resources_per_env}")
        print(f"- Lib files: {self.config.fixtures.total_lib_files} "
              f"(1 global + {self.config.fixtures.total_env_libs} env-specific)")
        print(f"- Total environments: {self.config.fixtures.total_envs}")
        print()

    def print_versions(self) -> None:
        """Print version information."""
        # tk outputs version to stderr
        tk_result = subprocess.run(
            ["tk", "--version"],
            capture_output=True,
            text=True,
        )
        tk_version = (tk_result.stdout or tk_result.stderr).strip()
        rtk_version = subprocess.run(
            [str(self.rtk), "--version"],
            capture_output=True,
            text=True,
        ).stdout.strip()

        print("### Versions")
        print()
        print(f"- tk: {tk_version}")
        print(f"- rtk: {rtk_version}")
        if self.rtk_base:
            rtk_base_version = subprocess.run(
                [str(self.rtk_base), "--version"],
                capture_output=True,
                text=True,
            ).stdout.strip()
            print(f"- rtk-base: {rtk_base_version}")
        print()
        print("</details>")
        print()

    def run(self) -> None:
        """Run the benchmark."""
        self.check_dependencies()
        self.build_rtk()
        self.build_rtk_base()

        self.print_header()
        self.print_versions()

        output_file = Path(os.environ.get("BENCHMARK_MARKDOWN_OUTPUT", tempfile.mktemp()))

        with tempfile.TemporaryDirectory() as tmpdir:
            self.generate_fixtures(Path(tmpdir))

            # Create separate export directories for each tool
            self.export_dir_tk = Path(tmpdir) / "export-output-tk"
            self.export_dir_tk.mkdir(exist_ok=True)
            self.export_dir_rtk = Path(tmpdir) / "export-output-rtk"
            self.export_dir_rtk.mkdir(exist_ok=True)
            if self.rtk_base:
                self.export_dir_rtk_base = Path(tmpdir) / "export-output-rtk-base"
                self.export_dir_rtk_base.mkdir(exist_ok=True)

            # Run setup if configured (e.g., pre-export for replace benchmarks)
            self.run_setup()

            print("Validating outputs match before benchmarking...", file=sys.stderr)
            for test in self.config.tests:
                self.validate_test(test)
            print(file=sys.stderr)

            print("## Benchmarks")
            print()

            # Run benchmarks and collect summaries
            summaries = []
            for i, test in enumerate(self.config.tests, 1):
                summary = self.run_benchmark(test, output_file, i)
                summaries.append(summary)

        # Write summary JSON for CI to parse
        summary_json_path = Path(os.environ.get("BENCHMARK_SUMMARY_OUTPUT", "benchmark-summary.json"))
        self._write_summary_json(summaries, summary_json_path)

        print(f"Markdown output written to: {output_file}", file=sys.stderr)
        print(f"Summary JSON written to: {summary_json_path}", file=sys.stderr)

    def _write_summary_json(self, summaries: list[dict], output_path: Path) -> None:
        """Write summary JSON file for CI to parse."""
        import json
        output = {
            "benchmark_name": self.config.name,
            "benchmark_id": self.config.id,
            "tests": summaries,
        }
        with open(output_path, "w") as f:
            json.dump(output, f, indent=2)


def main():
    parser = argparse.ArgumentParser(
        description="Run benchmarks from YAML config",
        usage="%(prog)s config [--rtk-base-path PATH] [-- hyperfine_args...]",
    )
    parser.add_argument("config", type=Path, help="Path to benchmark YAML config file")
    parser.add_argument("--rtk-base-path", type=Path, help="Path to pre-built rtk binary for baseline comparison")

    # Use parse_known_args to separate our args from hyperfine args
    args, hyperfine_args = parser.parse_known_args()

    # Remove leading '--' if present (used to separate our args from hyperfine args)
    if hyperfine_args and hyperfine_args[0] == "--":
        hyperfine_args = hyperfine_args[1:]

    repo_root = Path(__file__).parent.parent.resolve()
    config = BenchmarkConfig.from_yaml(args.config)
    runner = BenchmarkRunner(config, repo_root, hyperfine_args, rtk_base_path=args.rtk_base_path)
    runner.run()


if __name__ == "__main__":
    main()
