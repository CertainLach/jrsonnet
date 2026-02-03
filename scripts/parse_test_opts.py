#!/usr/bin/env python3
# /// script
# requires-python = ">=3.8"
# ///
"""
Parse test_opts.json and output CLI arguments for tk/rtk export.

This script reads test_opts.json from a test fixture directory and outputs
the corresponding CLI arguments that should be passed to both tk and rtk
for consistency testing.

Usage: parse_test_opts.py <fixture_dir> [--extension-only]
       --extension-only: Only output the extension value (default: golden)
"""

import json
import shlex
import sys
from pathlib import Path


def shell_quote(s: str) -> str:
    """Quote a string for safe shell usage."""
    return shlex.quote(s)


def parse_test_opts(fixture_dir: str, extension_only: bool = False) -> str:
    """Parse test_opts.json and return CLI arguments string."""
    opts_path = Path(fixture_dir) / "test_opts.json"

    # Default extension
    extension = "golden"

    if opts_path.exists():
        with open(opts_path) as f:
            opts = json.load(f)
        if "extension" in opts and opts["extension"]:
            extension = opts["extension"]
    else:
        opts = {}

    if extension_only:
        return extension

    args = []

    # --ext-code key=value (value needs shell quoting if it contains special chars)
    for key, value in opts.get("ext_code", {}).items():
        args.append(f"--ext-code={key}={shell_quote(value)}")

    # --ext-str key=value (or -V key=value)
    for key, value in opts.get("ext_str", {}).items():
        args.append(f"--ext-str={key}={shell_quote(value)}")

    # Note: extension is handled separately by the Makefile

    # --merge-deleted-envs (can be specified multiple times)
    for env in opts.get("merge_deleted_envs", []):
        args.append(f"--merge-deleted-envs={shell_quote(env)}")

    # --selector
    if "selector" in opts and opts["selector"]:
        args.append(f"--selector={shell_quote(opts['selector'])}")

    # --skip-manifest
    if opts.get("skip_manifest"):
        args.append("--skip-manifest")

    # --target (can be specified multiple times)
    for target in opts.get("target", []):
        args.append(f"--target={shell_quote(target)}")

    # --tla-code key=value (value needs shell quoting if it contains special chars)
    for key, value in opts.get("tla_code", {}).items():
        args.append(f"--tla-code={key}={shell_quote(value)}")

    # --tla-str key=value (or -A key=value)
    for key, value in opts.get("tla_str", {}).items():
        args.append(f"--tla-str={key}={shell_quote(value)}")

    return " ".join(args)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(
            f"Usage: {sys.argv[0]} <fixture_dir> [--extension-only]", file=sys.stderr)
        sys.exit(1)

    fixture_dir = sys.argv[1]
    extension_only = "--extension-only" in sys.argv

    print(parse_test_opts(fixture_dir, extension_only))
