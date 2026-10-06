#!/usr/bin/env python3
"""Public synthetic retrieval probe; calls only the explicitly selected encoder.

Usage: python3 scripts/compare_story_retrieval.py target/debug/yomibu \
    --embedding-provider lexical-baseline
All arguments after the binary are forwarded to prepare-retrieval. Hosted work
requires its normal --allow-embedding-call opt-in. No generation is performed.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def main():
    binary = Path(sys.argv[1]).resolve()
    embedding_args = sys.argv[2:]
    suite = json.loads((Path(__file__).resolve().parents[1] /
                        "tests/fixtures/story/retrieval-cases.json").read_text())
    rows = []
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        inventory = root / "inventory.json"
        request = root / "request.json"
        cache = root / "vectors.json"
        inventory.write_text(json.dumps(suite["inventory"], ensure_ascii=False))
        args = ["--inventory", str(inventory), "--request", str(request),
                "--embedding-cache", str(cache), "--select", "3"]
        for case in suite["cases"]:
            request.write_text(json.dumps({"version": 1, "brief": case["brief"],
                                          "targets": {"vocabulary": [], "grammar": []}}))
            started = time.monotonic()
            subprocess.run([str(binary), "prepare-retrieval", *args, *embedding_args],
                           check=True, stdout=subprocess.DEVNULL)
            elapsed = time.monotonic() - started
            preview = json.loads(subprocess.check_output(
                [str(binary), "preview-story", *args, "--json"]))
            selected = [s["word"]["id"] for s in preview["plan"]["selected"]]
            hits = len(set(selected).intersection(case["relevant"]))
            rows.append({"brief": case["brief"], "top3": selected,
                         "recall_at_3": hits / len(case["relevant"]),
                         "prepare_seconds": round(elapsed, 6)})
        print(json.dumps({"model": preview["plan"]["embedding_model"], "cases": rows,
                          "mean_recall_at_3": sum(r["recall_at_3"] for r in rows) / len(rows)},
                         ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
