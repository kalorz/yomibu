#!/usr/bin/env python3
"""Public synthetic retrieval probe; calls only the explicitly selected encoder.

Usage: python3 scripts/compare_story_retrieval.py target/debug/yomibu \
    --embedding-provider lexical-baseline
All arguments after the binary are forwarded to prepare-retrieval. Hosted work
requires its normal --allow-embedding-call opt-in. No generation is performed.
"""
import json
import os
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
    environment = {name: value for name, value in os.environ.items()
                   if not name.startswith("YOMIBU_") or name in
                   {"YOMIBU_HTTP_EMBEDDINGS_API_KEY", "YOMIBU_CREDENTIAL_OPENAI"}}
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        inventory = root / "inventory.json"
        request = root / "request.json"
        cache = root / "vectors.json"
        inventory.write_text(json.dumps(suite["inventory"], ensure_ascii=False))
        args = ["--data-dir", str(root / "data"), "--inventory", str(inventory),
                "--request", str(request),
                "--embedding-cache", str(cache), "--select", "3"]
        for case in suite["cases"]:
            request.write_text(json.dumps({"version": 1, "topic": case["topic"],
                                          "targets": {"vocabulary": [], "grammar": []}}))
            started = time.monotonic()
            subprocess.run([str(binary), "prepare-retrieval", *args, *embedding_args],
                           check=True, stdout=subprocess.DEVNULL, env=environment)
            elapsed = time.monotonic() - started
            preview = json.loads(subprocess.check_output(
                [str(binary), "preview-story", *args, "--enable", "embeddings", "--json"], env=environment))
            selected = preview["selection"]["vocabulary_ids"]
            hits = len(set(selected).intersection(case["relevant"]))
            rows.append({"topic": case["topic"], "top3": selected,
                         "recall_at_3": hits / len(case["relevant"]),
                         "prepare_seconds": round(elapsed, 6)})
        print(json.dumps({"model": preview["selection"]["embedding_model"], "cases": rows,
                          "mean_recall_at_3": sum(r["recall_at_3"] for r in rows) / len(rows)},
                         ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
