#!/usr/bin/env python3
"""Check the workspace's declared crate dependency boundaries."""

import argparse
import json
from pathlib import Path
import subprocess
import sys


REQUIRED_CRATES = {"yomibu-core", "yomibu-app", "yomibu-ext", "yomibu-cli"}
DEPENDENCIES = {
    "yomibu-core": set(),
    "yomibu-app": {"yomibu-core"},
    "yomibu-ext": {"yomibu-core"},
    "yomibu-cli": {"yomibu-core", "yomibu-app", "yomibu-ext"},
    "yomibu-web": {"yomibu-core", "yomibu-app", "yomibu-ext"},
    "yomibu-mcp": {"yomibu-core", "yomibu-app", "yomibu-ext"},
}
# Existing application integration tests use real extension implementations.
# This is not permission for a production or build dependency on extensions.
REVIEWED_DEV_DEPENDENCIES = {("yomibu-app", "yomibu-ext")}


def violations(metadata):
    workspace_ids = set(metadata["workspace_members"])
    packages = {
        package["name"]: package
        for package in metadata["packages"]
        if package["id"] in workspace_ids
    }
    errors = [
        f"Missing retained workspace crate: {name}"
        for name in sorted(REQUIRED_CRATES - packages.keys())
    ]
    for name, package in sorted(packages.items()):
        if name not in DEPENDENCIES:
            errors.append(f"Unclassified workspace crate: {name}; review its architectural role")
            continue
        for dependency in package["dependencies"]:
            target = dependency["name"]  # Cargo keeps the package name even for renamed imports.
            if target not in packages:
                continue
            kind = dependency["kind"] or "normal"
            allowed = target in DEPENDENCIES[name]
            if kind == "dev" and (name, target) in REVIEWED_DEV_DEPENDENCIES:
                allowed = True
            if not allowed:
                details = kind
                if dependency["target"]:
                    details += f", target: {dependency['target']}"
                if dependency["rename"]:
                    details += f", imported as: {dependency['rename']}"
                errors.append(f"Forbidden workspace dependency: {name} -> {target} ({details})")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest-path", type=Path,
        default=Path(__file__).resolve().parents[1] / "Cargo.toml",
    )
    args = parser.parse_args()
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps",
             "--all-features", "--locked", "--offline",
             "--manifest-path", str(args.manifest_path)],
            text=True, capture_output=True, timeout=60,
        )
        if result.returncode:
            print(f"Cargo metadata failed:\n{result.stderr}", file=sys.stderr)
            return 2
        metadata = json.loads(result.stdout)
        errors = violations(metadata)
    except (OSError, subprocess.TimeoutExpired, ValueError, KeyError, TypeError) as error:
        print(f"Architecture check could not run: {error}", file=sys.stderr)
        return 2
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Crate dependency checks passed ({len(metadata['workspace_members'])} workspace crates).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
