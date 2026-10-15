"""Run Bylaw; apply exact, temporary exceptions only to the domain boundary rule."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys


def check_baseline(root, envelope, baseline):
    if envelope["version"] != 1:
        raise ValueError("Unsupported Bylaw report version")
    report = envelope["report"]
    if report["analysis_diagnostics"]:
        raise ValueError(f"Incomplete Bylaw analysis: {report['analysis_diagnostics']}")
    rules = report["rule_results"]
    if (sorted((rule["rule_id"], rule["severity"]) for rule in rules)
            != [("domain-boundary", "error"), ("inner-libraries", "error")]):
        raise ValueError("Expected domain-boundary and inner-libraries rules at error severity")
    rules = {rule["rule_id"]: rule for rule in rules}
    # Declared library edges can have no source span and have no exceptions.
    errors = [v["message"] for v in rules["inner-libraries"]["violations"]]
    findings = {}
    for violation in rules["domain-boundary"]["violations"]:
        evidence = violation["evidence"]
        if not evidence or any(item["span"] is None for item in evidence):
            raise ValueError(f"Unlocated violation: {violation['message']}")
        # Bylaw 0.1.0 records namespace prefixes of grouped imports as edges.
        # Ignore only that namespace reference, never items owned by the root.
        target = violation["target"]
        if target["kind"] == "module" and target["id"].endswith("::yomibu_core"):
            evidence = [item for item in evidence if not all(
                part in ("crate", "super", "self")
                for part in (item["description"] or "").split("::"))]
        for item in evidence:
            relative = Path(item["span"]["path"]).resolve().relative_to(root).as_posix()
            findings.setdefault(relative, set()).add(violation["message"])
    for file in sorted(findings.keys() | baseline.keys()):
        actual = sorted(findings.get(file, []))
        if file not in baseline:
            errors.extend(f"{file}: {finding}" for finding in actual)
        elif not actual:
            errors.append(f"{file}: stale baseline; remove the fixed/deleted entry")
        elif (actual != baseline[file]["violations"]
              or hashlib.sha256((root / file).read_bytes()).hexdigest()
              != baseline[file]["sha256"]):
            errors.append(f"{file}: baselined source or findings changed; resolve the violations or obtain explicit review")
    return errors


def main():
    root = Path(__file__).resolve().parents[1]
    try:
        result = subprocess.run(
            ["cargo", "bylaw", "check", "--format", "json"],
            cwd=root, text=True, capture_output=True, timeout=300,
        )
        if result.returncode not in (0, 1):
            raise ValueError(result.stderr or result.stdout)
        baseline = json.loads((root / "tools/domain-baseline.json").read_text())
        errors = check_baseline(root, json.loads(result.stdout), baseline)
    except (OSError, subprocess.TimeoutExpired, ValueError, KeyError, TypeError) as error:
        print(f"Dependency check could not run: {error}", file=sys.stderr)
        return 2
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Dependency rules passed ({len(baseline)} frozen domain exceptions).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
