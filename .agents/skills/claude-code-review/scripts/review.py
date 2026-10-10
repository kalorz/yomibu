import argparse
import json
import math
import os
import shutil
import signal
import subprocess
import tempfile
from pathlib import Path


def read_review(path, model):
    selected_model = None
    result = None
    with path.open() as stream:
        for line in stream:
            if not line.strip():
                continue
            event = json.loads(line)
            if not isinstance(event, dict):
                raise ValueError("Invalid Claude protocol event")
            if event.get("type") == "system" and event.get("subtype") == "init":
                selected_model = event.get("model")
            if event.get("type") == "result":
                result = event
    if result is None:
        raise ValueError("Claude returned no final result")
    if result.get("is_error") is not False:
        raise ValueError("Claude reported an error")
    denials = result.get("permission_denials")
    if not isinstance(denials, list) or denials:
        raise ValueError("Review has denied or unverified tool permissions")
    usage = result.get("modelUsage")
    if selected_model != model or not isinstance(usage, dict) or set(usage) != {model}:
        raise ValueError("Requested model was not verified for the whole review")
    report = result.get("result")
    if not isinstance(report, str) or not report.strip():
        raise ValueError("Claude returned no review report")
    return result


def local_target(value):
    if not value.strip() or any(ord(char) < 32 or ord(char) == 127 for char in value):
        raise argparse.ArgumentTypeError("Target must be a nonempty, single-line ref or path")
    if any(token.startswith("-") for token in value.split()):
        raise argparse.ArgumentTypeError("Target cannot contain Claude command flags")
    return value


def deadline(value):
    seconds = float(value)
    if not math.isfinite(seconds) or seconds <= 0:
        raise argparse.ArgumentTypeError("Timeout must be a positive number of seconds")
    return seconds


def stop_review(process):
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return process.wait()
    try:
        return process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        return process.wait()


def main():
    parser = argparse.ArgumentParser(description="Run a fresh, read-only Claude Code review")
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--target", type=local_target, help="Base ref, ref range, or local path")
    parser.add_argument("--model", default="claude-opus-5-5", help="Full model ID")
    parser.add_argument(
        "--effort", choices=("low", "medium", "high", "xhigh", "max"), default="xhigh"
    )
    parser.add_argument("--timeout", type=deadline, default=600, help="Seconds; default 600")
    options = parser.parse_args()
    repo = options.repo.resolve()
    if not repo.is_dir():
        parser.error("Repository directory does not exist")
    executable = shutil.which("claude")
    if executable is None:
        parser.error("Claude Code is not installed on PATH")

    artifacts = Path(tempfile.mkdtemp(prefix="claude-code-review-"))
    prompt = f"/code-review {options.effort}"
    if options.target:
        prompt += f" {options.target}"
    allowed = [
        "Read", "Glob", "Grep", "Agent", "Skill",
        "Bash(git diff)", "Bash(git diff *)", "Bash(git status)", "Bash(git status *)",
        "Bash(git log *)", "Bash(git show *)", "Bash(git rev-parse *)",
        "Bash(git ls-files *)", "Bash(git remote -v)",
        "Bash(ls)", "Bash(ls *)", "Bash(head)", "Bash(head *)", "Bash(echo *)", "Bash(cat *)",
    ]
    command = [
        executable, "-p", prompt, "--model", options.model, "--effort", options.effort,
        "--output-format", "stream-json", "--verbose", "--forward-subagent-text",
        "--no-session-persistence", "--no-chrome", "--strict-mcp-config",
        "--mcp-config", '{"mcpServers":{}}',
        "--settings", '{"disableAllHooks":true,"autoMemoryEnabled":false}',
        "--permission-mode", "plan", "--permission-prompts", "none",
        "--tools", "Read,Glob,Grep,Agent,Skill,Bash", "--allowedTools", ",".join(allowed),
        "--disallowedTools",
        "Edit,Write,NotebookEdit,Bash(gh *),Bash(git push *),"
        "Bash(git commit *),Bash(git add *)",
        "--append-system-prompt",
        "Run a local, read-only review. Read repository AGENTS.md and relevant linked contracts. "
        "Do not edit files, run builds or tests, install packages, change configuration, create "
        "commits, publish, or post comments. The host runs validation.",
    ]
    (artifacts / "command.json").write_text(json.dumps(command, indent=2))
    summary = {
        "status": "incomplete", "model": options.model, "effort": options.effort,
        "repo": str(repo), "artifacts": str(artifacts),
    }
    exit_code = 1
    try:
        with (
            (artifacts / "events.jsonl").open("w") as stdout,
            (artifacts / "stderr.log").open("w") as stderr,
        ):
            process = subprocess.Popen(
                command, cwd=repo, stdin=subprocess.DEVNULL,
                stdout=stdout, stderr=stderr, start_new_session=True,
            )
            try:
                return_code = process.wait(timeout=options.timeout)
            except subprocess.TimeoutExpired:
                stop_review(process)
                exit_code = 124
                raise ValueError("Claude review timed out") from None
            except KeyboardInterrupt:
                stop_review(process)
                exit_code = 130
                raise ValueError("Claude review was interrupted") from None
        summary["claude_exit_code"] = return_code
        if return_code != 0:
            raise ValueError(f"Claude exited with status {return_code}; inspect the saved logs")
        result = read_review(artifacts / "events.jsonl", options.model)
        (artifacts / "result.json").write_text(json.dumps(result, indent=2, ensure_ascii=False))
        (artifacts / "report.txt").write_text(result["result"])
        summary.update(
            status="complete", report=result["result"],
            estimated_cost_usd=result.get("total_cost_usd"),
        )
        exit_code = 0
    except (OSError, ValueError) as error:
        summary["error"] = str(error)
    encoded = json.dumps(summary, indent=2, ensure_ascii=False)
    (artifacts / "summary.json").write_text(encoded)
    print(encoded)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
