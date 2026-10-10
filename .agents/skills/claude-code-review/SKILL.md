---
name: claude-code-review
description: Independently review code changes through Claude Code with Opus 5.5 and xhigh. Use for requested Claude reviews or before delivering substantive code changes.
---

# Claude Code review

Use built-in `/code-review`; the PR plugin is `code-review:code-review`.
Override model or effort only on explicit user request. Never enable fixes or
comment posting.

Finish repository checks. For committed changes, pass a verified base ref or
range with `--target`; omit it for local changes. Give Claude rules and
requirements without seeding suspected bugs or prior conclusions.

For cloud setup, read [references/cloud.md](references/cloud.md).
Run [scripts/review.py](scripts/review.py) in a fresh session:

```sh
python3 <skill-dir>/scripts/review.py --repo <repository>
```

The runner disables hooks, MCP, persistence and edit tools, and saves temporary
logs. Use yielding execution calls and share progress. If the Codex sandbox
blocks keychain login or network access, use normal execution escalation. Never
copy credentials or bypass Claude permissions.

Nonzero exits and `status: incomplete` mean an unfinished review; inspect logs
and report the limitation. Exit zero means completion, including when bugs were
found. Verify findings against code, fix confirmed issues under repository
rules, then rerun affected checks and reviews. Report unresolved findings.
