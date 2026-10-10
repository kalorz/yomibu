import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from review import read_review


def parse(events):
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "review.jsonl"
        path.write_text("".join(json.dumps(event) + "\n" for event in events))
        return read_review(path, "claude-opus-5-5")


def successful_events():
    return [
        {"type": "system", "subtype": "init", "model": "claude-opus-5-5"},
        {
            "type": "result",
            "is_error": False,
            "result": "Found a UTF-8 boundary bug.",
            "permission_denials": [],
            "modelUsage": {"claude-opus-5-5": {"outputTokens": 100}},
        },
    ]


class ReviewResultTests(unittest.TestCase):
    def test_completed_review_returns_findings_and_verified_model(self):
        result = parse(successful_events())
        self.assertEqual(result["result"], "Found a UTF-8 boundary bug.")

    def test_denied_diff_access_makes_the_review_incomplete(self):
        events = successful_events()
        events[-1]["permission_denials"] = [{"tool_name": "Bash"}]
        with self.assertRaisesRegex(ValueError, "permission"):
            parse(events)

    def test_claude_error_is_rejected_even_with_a_result(self):
        events = successful_events()
        events[-1]["is_error"] = True
        with self.assertRaisesRegex(ValueError, "error"):
            parse(events)

    def test_different_or_unverified_models_are_rejected(self):
        for selected, used in (
            ("claude-sonnet-5-5", "claude-sonnet-5-5"),
            ("claude-opus-5-5", "claude-sonnet-5-5"),
            (None, "claude-opus-5-5"),
        ):
            with self.subTest(selected=selected, used=used):
                events = successful_events()
                events[0]["model"] = selected
                events[-1]["modelUsage"] = {used: {"outputTokens": 100}}
                with self.assertRaisesRegex(ValueError, "model"):
                    parse(events)

    def test_missing_result_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "result"):
            parse(successful_events()[:1])

    def test_empty_report_is_rejected(self):
        events = successful_events()
        events[-1]["result"] = ""
        with self.assertRaisesRegex(ValueError, "report"):
            parse(events)

    def test_non_object_protocol_event_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "event"):
            parse([[]] + successful_events())


class ReviewCommandTests(unittest.TestCase):
    def run_fixture(self, options=(), exit_code=0, sleep=0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "claude"
            executable.write_text(
                f"#!{sys.executable}\n"
                "import json, sys, time\n"
                "from pathlib import Path\n"
                "Path('invocation.json').write_text(json.dumps(sys.argv[1:]))\n"
                f"events = {successful_events()!r}\n"
                "for event in events: print(json.dumps(event), flush=True)\n"
                f"time.sleep({sleep!r})\n"
                f"sys.exit({exit_code})\n"
            )
            executable.chmod(0o755)
            environment = dict(os.environ)
            environment["PATH"] = directory + os.pathsep + environment.get("PATH", "")
            environment["TMPDIR"] = directory
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name("review.py")),
                 "--repo", directory, *options],
                env=environment, capture_output=True, text=True, timeout=5,
            )
            invocation = root / "invocation.json"
            args = json.loads(invocation.read_text()) if invocation.exists() else None
            return result, args

    def test_review_pins_opus_xhigh_and_exposes_findings_without_write_tools(self):
        result, args = self.run_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        summary = json.loads(result.stdout)
        self.assertEqual(summary["status"], "complete")
        self.assertEqual(summary["report"], "Found a UTF-8 boundary bug.")
        self.assertEqual(args[args.index("-p") + 1], "/code-review xhigh")
        self.assertEqual(args[args.index("--model") + 1], "claude-opus-5-5")
        self.assertEqual(args[args.index("--effort") + 1], "xhigh")
        self.assertEqual(args[args.index("--permission-mode") + 1], "plan")
        tools = args[args.index("--tools") + 1].split(",")
        self.assertNotIn("Edit", tools)
        self.assertNotIn("Write", tools)
        self.assertNotIn("--dangerously-skip-permissions", args)

    def test_explicit_scope_and_effort_are_passed_to_the_builtin_review(self):
        result, args = self.run_fixture(("--target", "main...HEAD", "--effort", "high"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(args[args.index("-p") + 1], "/code-review high main...HEAD")

    def test_nonzero_claude_exit_cannot_be_reported_as_complete(self):
        result, _ = self.run_fixture(exit_code=3)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(json.loads(result.stdout)["status"], "incomplete")

    def test_timeout_cannot_be_reported_as_complete_even_with_a_result(self):
        result, _ = self.run_fixture(("--timeout", "0.1"), sleep=10)
        self.assertEqual(result.returncode, 124)
        self.assertEqual(json.loads(result.stdout)["status"], "incomplete")

    def test_targets_cannot_enable_fixes_posting_or_inject_another_command(self):
        for target in ("main --fix", "main --comment", "main\n/simplify"):
            with self.subTest(target=target):
                result, args = self.run_fixture(("--target", target))
                self.assertEqual(result.returncode, 2)
                self.assertIsNone(args)


if __name__ == "__main__":
    unittest.main()
