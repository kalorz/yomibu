"""Run Bylaw with a narrow correction for grouped-import namespace evidence."""

import json
from pathlib import Path
import subprocess
import sys


def check_report(envelope):
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
        if evidence:
            errors.append(violation["message"])
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
        errors = check_report(json.loads(result.stdout))
    except (OSError, subprocess.TimeoutExpired, ValueError, KeyError, TypeError) as error:
        print(f"Dependency check could not run: {error}", file=sys.stderr)
        return 2
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Dependency rules passed (no exceptions).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
