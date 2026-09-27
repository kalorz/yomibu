# Yomibu implementation plan

`SPEC.md` is the authoritative product and architecture baseline. This document
tracks delivery order and acceptance criteria; it does not authorize future
milestones merely by listing them.

## Current state and stopping point

The documentation foundation is the reviewed baseline. No application code,
Cargo package, dependencies, or CI workflow has been initialized. This
documentation task ends with `SPEC.md`, `PLAN.md`, and `AGENTS.md`; the next
implementation step is 1a, offline status, using strict TDD for behavior.

Inspection on 2026-09-27 found an empty repository on `main`, no commits, and a
configured `origin` remote. The installed default Rust and Cargo were 1.82.0.
The verified stable release was 1.98.1, with edition 2024. Updating is recommended
before initialization; no toolchain update is part of the documentation task.

To install the stable toolchain alongside the existing one:

```sh
rustup update stable
rustup component add --toolchain stable rustfmt clippy
rustc +stable --version
```

During project initialization, verify and pin the current stable version as
specified in `SPEC.md`. These instructions do not imply that the update has been
performed.

## Vertical milestones

| Step | Status | Deliverable | Acceptance |
| --- | --- | --- | --- |
| 0 — Decisions | Complete | Three authoritative, consistent documents | Sourced external facts; proposed structure; Rust rationale; no application initialization |
| 1a — Offline status | Not started | Package, domain snapshot, cache reader, real `status` command | Fixture-backed integration tests; no token/network dependency; useful errors and summaries |
| 1b — First complete sync | Not started | HTTP adapter, normalization, safe persistence, thin CLI composition | Mock API → normalized snapshot → disk → offline status succeeds through real components |
| 1c — Sync resilience | Not started | Pagination, rate limits, deadlines, integrity validation, account protection, locking | Failure cases preserve a usable complete cache; concurrent access and post-replacement errors behave as specified |
| 1d — Milestone acceptance | Not started | macOS/Linux CI and reviewed public library surface | All quality gates pass; documented limitations; no placeholder future features |

Implementation steps use small Red-Green-Refactor cycles (see `AGENTS.md`). Tests
accompany behavior, beginning with a confirmed failing test, rather than being
added after implementation or postponed to 1c.

Basic credential protection, timeouts, and safe persistence apply as soon as the
respective I/O is introduced; 1c completes and exercises the failure paths rather
than retrofitting unsafe foundations.

### 1a — Offline status

- Initialize one package with library and thin binary, the verified stable
  toolchain pin, edition 2024, and committed lockfile.
- Introduce domain structures and the versioned cache reader. Normalize absence
  and subject variants explicitly; preserve source state without a "known" policy.
- Implement real offline summaries and CLI status, including `--data-dir`.
- Exercise fixtures through production loading and summary code. Fixtures are
  test inputs, not a fake application data source.
- Verify empty accounts, missing/corrupt/unsupported caches, and correct aggregate
  review arithmetic. Status must work without credentials or any network access.

### 1b — First complete sync

- Fetch and normalize learner identity, progress, statistics, and associated
  subjects using the actual HTTP adapter against a local mock server.
- Implement full snapshot replacement, account protection, and safe cache writes.
- Keep command composition in the binary and useful behavior callable from the
  library. Do not leak transport DTOs into the public API.
- Verify the complete retrieval-to-cache-to-status flow, including repeated syncs.

### 1c — Sync resilience

- Complete pagination, bounded retry behavior, deadlines, response limits,
  credential-safe URL handling, and structural integrity checks.
- Exercise reset/removal replacement, permitted absent data, content-access
  exclusions, writer contention, and interruptions.
- Verify old-cache preservation on every tested pre-replacement failure. Test
  post-replacement durability errors separately rather than asserting rollback.

### 1d — Milestone acceptance

- Run the quality gates on macOS and Linux and enforce the lockfile in CI.
- Review the public library surface, dependency features, secret handling, error
  messages, and alignment with `SPEC.md`.
- Document actual validation results and remaining limitations. Do not make a live
  account or a real API request a prerequisite for the automated test suite.

## Test strategy

| Technique | Meaningful scenarios |
| --- | --- |
| Unit tests | Domain validation, nullable state, subject variants, summary counts, weighted accuracy, zero denominators |
| Local HTTP adapter tests | Headers, success, multiple/empty pages, kana-only vocabulary, permitted partial data, additive fields, malformed JSON, invalid required fields, authentication errors, rate limiting, transient/permanent failures, timeouts, hostile pagination URLs, oversized responses, duplicate records, missing subjects |
| Storage integration tests | Round trips, Unicode/timestamps, schema rejection, corrupt/truncated files, interrupted writes, atomic replacement, writer contention, account mismatch, errors after replacement |
| Cross-component tests | Complete sync, repeat sync, reset/removal replacement, access exclusions, later-page failure leaving the previous cache unchanged |
| Black-box CLI tests | Help, argument validation, missing token, alternate data directory, status without credentials, errors, exit codes, secret-free output |
| Property tests | Input-order independence, count conservation across subject partitions, bounded aggregate accuracy, normalized-data serialization round trips |
| Golden tests | Stable status output with fixed timestamps only |

Use synthetic but realistic fixtures grounded in the official API contract.
Preserve their provenance without storing real credentials or private learner
data. Isolate servers and temporary directories per test. Use controlled clocks
and deterministic property-test seeds; retain regression cases for non-trivial
bugs. Do not use slow wall-clock sleeps to test retry schedules where controlled
time is sufficient.

Normal tests never call the real WaniKani API. Local loopback mock-server traffic
is allowed. Do not mock pure domain logic or mutate process-global environment
variables to configure parallel tests; pass environment overrides to individual
CLI child processes.

Test behavior and invariants rather than copying implementation logic. Do not add
snapshots, property tests, or coverage targets solely to increase a metric.

## Validation commands

For the documentation-only milestone:

- Review the three documents for consistent decisions, sources, and stopping scope.
- Confirm that only the requested documents were created.
- Check whitespace with `git diff --check`; include untracked new files in the
  review rather than assuming a normal diff covers them.
- Do not run Cargo quality gates before a Cargo package exists.

Before declaring an implementation milestone complete, run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

CI runs equivalent checks on macOS and Linux with lockfile enforcement:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
```

Use additional tooling only when it answers a concrete concern. Report which
checks actually ran and their results. No Cargo checks have been run at the
documentation stage because no Rust project exists.

## Later milestones

These require separate design work and are not part of milestone 1:

1. **Learner constraints and retrieval:** grammar knowledge as learner data,
   initially entered through a local file, an explicit revisable knowledge
   policy, manual targets, and structured lexical retrieval. Targets identify
   the word, intended reading, and intended sense. Acceptance: explainable
   target/context selection from real learner data with no mandatory vector
   search. When database persistence is introduced, grammar belongs alongside
   other learner data; files may remain import/export.
2. **Validated generation:** deterministic Japanese validation, followed by real
   best-of-two generation and bounded repair for simple sentences and short
   stories. Include focused, grounded sense/reading checks when supporting
   ambiguous targets. Acceptance: select a valid passage or report failure
   without silently relaxing constraints; length and complexity do not authorize
   unfamiliar vocabulary or kanji.
3. **Reading practice:** reading quizzes with kana/romaji normalization and
   persisted attempts. Acceptance: deterministic comparison against contextually
   validated readings and useful mistake records. Rephrase or reject ambiguous
   prompts; do not penalize another contextually valid reading solely because it
   differs from the intended target.
4. **Meaning and usage evaluation:** multilingual translation judging and grounded
   semantic criticism beyond the focused target checks already introduced.
   Acceptance: evaluate preserved meaning without requiring one canonical
   translation; assess broader naturalness and grammar suitability.
5. **Adaptive selection:** combine WaniKani statistics with Yomibu mistakes.
   Acceptance: target choices use both sources and can be explained without
   introducing an SRS.

Bunpro, JMdict, external corpora, web/native/MCP interfaces, PostgreSQL, and vector
retrieval remain conditional future work. Revisit them only when a concrete
product or retrieval need justifies their cost.
