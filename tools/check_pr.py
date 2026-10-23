#!/usr/bin/env python3
"""Report a pull request's size and refuse guide edits mixed with crate changes."""

import re
import subprocess
import sys

GUIDES = {"AGENTS.md", "ARCHITECTURE.md"}
PROSE = GUIDES | {"GLOSSARY.md", "README.md"}
PUBLIC_ITEM = re.compile(
    r"^\+\s*pub(\([^)]*\))?\s+(?:(?:const|async|unsafe)\s+)*(fn|struct|enum|trait|type|mod|const|static)\b"
)
# Soft limits: a warning asks for a sentence of justification in the PR, nothing more.
SOFT_LIMITS = {"Rust lines changed": 400, "Guide words added": 300}


def git(*args):
    return subprocess.run(["git", *args], text=True, capture_output=True, check=True).stdout


def added_lines(diff):
    return [line[1:] for line in diff.splitlines() if line.startswith("+") and not line.startswith("+++")]


def main(base, head):
    # --no-renames makes a renamed guide or crate file count as a delete and an add.
    span = f"{base}...{head}"
    changed = {}
    for line in git("diff", "--no-renames", "--numstat", span).splitlines():
        added, deleted, path = line.split("\t")
        changed[path] = sum(int(n) for n in (added, deleted) if n != "-")
    rust = {path: lines for path, lines in changed.items() if path.endswith(".rs")}
    tests = sum(lines for path, lines in rust.items() if "/tests/" in path)
    public = sum(1 for line in git("diff", "--no-renames", "--unified=0", span, "--", "*.rs").splitlines()
                 if PUBLIC_ITEM.match(line))
    words = sum(len(line.split()) for line in
                added_lines(git("diff", "--no-renames", "--unified=0", span, "--", *sorted(PROSE))))
    measures = {
        "Files changed": len(changed),
        "Rust lines changed": sum(rust.values()),
        "Rust lines in tests/": tests,
        "Public items added": public,
        "Guide words added": words,
    }
    print("| Measure | Value |\n| --- | --- |")
    for name, value in measures.items():
        print(f"| {name} | {value} |")
    for name, limit in SOFT_LIMITS.items():
        if measures[name] > limit:
            print(f"::warning::{name} is {measures[name]}, above {limit}; the PR should say why.")
    guides = sorted(GUIDES & changed.keys())
    if guides and any(path.startswith("crates/") for path in changed):
        print(f"::error::{', '.join(guides)} changed together with crate code; "
              "guides change only in their own PR.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:3]))
