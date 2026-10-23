# Repository guide

Read [ARCHITECTURE.md](ARCHITECTURE.md) before placing or changing code and
[GLOSSARY.md](GLOSSARY.md) before naming domain concepts. [README.md](README.md)
holds setup and how to start; the CLI documents its own commands. Keep the files in
`docs/plan/`; they are history and proposals, not requirements.

## How work is done

- The ticket is the specification. One behaviour per PR. No plan documents in the repo.
- State the expected size before starting. The usual size is 150 to 200 changed lines
  including tests; a ticket may justify more. If the diff reaches twice the stated
  expectation, stop and report before continuing.
- Write the failing test first, then the smallest production change, then simplify
  the whole diff once. There is no separate simplification pass afterwards.
- Every PR carries a short walkthrough for the reviewer: what it does, why each piece
  sits in its layer, and the Rust features worth noticing. Explain a concept the first
  time it appears, in the PR text, never in code comments.
- Name any new dependency in the PR and say what it does that we should not write ourselves.

## Decisions that need the user

Ask before any of these. Decide everything else and show it.

- a new crate
- a new or changed port, or a new public application operation
- a new domain term or a changed meaning
- a guarantee listed under [triggers](ARCHITECTURE.md#triggers)
- a change to any rule or check
- exceeding the size expectation

## Terminology

Use glossary terms in code, tests and prose. Translate provider terms at the adapter.
Record a new or changed term in the glossary before using it in code.

## Review

- A required finding points to a failing test, a broken rule or a wrong behaviour.
  "Could be more robust" and "for future flexibility" are not findings.
- Apply an accepted finding across the touched path, not only at the cited line.
- Review feedback does not authorize new behaviour or guarantees.

## Guidance changes

- AGENTS.md and ARCHITECTURE.md change only in their own PR, never with a feature.
- A new rule needs a mistake that happened twice and cannot become a check. Prefer a
  test or CI check. Each addition removes at least as many words.
- Never weaken a check or widen an allowlist to accommodate code.

## Verification and reporting

- Run the checks in [CI](.github/workflows/ci.yml) before handing over. Report what
  actually ran and what did not. Never substitute another backend to make a test pass.
- Commit or push only when asked.
