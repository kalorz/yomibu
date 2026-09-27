# Yomibu implementation plan

`SPEC.md` is the authoritative product and architecture baseline. This document
tracks delivery order and acceptance criteria; it does not authorize future
milestones merely by listing them.

## Current state and stopping point

Milestone **1a — Offline status is complete**. The package has a library and thin
CLI, a schema-1 cache reader, validated domain data, and offline summaries. Stop
here: synchronization, cache writing/locking, HTTP dependencies, and CI remain in
1b–1d. Only `status` is implemented; no placeholder `sync` command is present.

On 2026-09-27, the official Rust release page and `rustup update stable` both
confirmed Rust 1.98.1. Installed the exact 1.98.1 toolchain with rustfmt and Clippy,
and pinned it in `rust-toolchain.toml`, using edition 2024. The repository pin
selects Rust/Cargo 1.98.1 without changing the user's global 1.82.0 default.
`Cargo.lock` records the resolved dependencies; `tempfile` is development-only in
1a. No reqwest, Tokio, wiremock, or proptest dependency was needed.

## Vertical milestones

| Step | Status | Deliverable | Acceptance |
| --- | --- | --- | --- |
| 0 — Decisions | Complete | Three authoritative, consistent documents | Sourced external facts; proposed structure; Rust rationale; no application initialization |
| 1a — Offline status | Complete | Package, domain snapshot, cache reader, real `status` command | Fixture-backed integration tests; no token/network dependency; useful errors and summaries |
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

#### 1a delivery and TDD record

Each cycle below ran its tests before implementing the behavior, observed the
expected failure, implemented the change, confirmed GREEN, explicitly reviewed
production and test code, and reran the focused suite after that review. The
initial package/toolchain and empty targets were non-behavioral scaffolding.

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Cache loading | Missing cache API prevented the empty-account test from compiling | Loaded the fixture with Unicode, parsed timestamps, and nullable learner state; reviewed ownership/naming, no refactor justified |
| Cache errors/version | Four tests exposed generic I/O/JSON errors and payload decoding before version rejection | Added distinct typed errors with remedies and version-first decoding; reviewed error paths and kept path clones confined to failures |
| Retained source state | Mixed-fixture test failed to compile because subjects/progress and variants were absent | Added the three lexical shapes, assignments, statistics, and explicit content exclusions; reviewed absence and modelling, no abstraction justified |
| Cache integrity | All supplied invalid-value, reference, and duplicate mutations were incorrectly accepted | Added synchronous validation; refactored test mutation checks to remove panic-catching, then reran all cache tests |
| Counts | Summary API absent | Empty and mixed counts passed; reviewed deduplication, borrowed metadata, and inclusive vocabulary counts, no change justified |
| SRS grouping | SRS summary field absent | Grouped raw stages by optional system ID; reviewed deterministic ordering and unknown-system handling, retained concrete ordered maps |
| Accuracy | Aggregate accuracy API absent | Counter-weighted accuracy, independent zero denominators, and large counters passed; reviewed widening and encapsulated totals, no change justified |
| CLI status | No-op binary emitted nothing and incorrectly succeeded on cache errors | Real fixture-backed output and nonzero errors passed; reviewed presentation/library separation and shared accuracy rendering, no further change justified |
| HOME resolution | CLI required an explicit path even with HOME and lacked the unavailable-HOME remedy | Added lazy OS-native HOME fallback and rejected empty HOME; reviewed precedence and subprocess environment isolation, no change justified |
| Reading classification regression | Final review found that a blank kanji reading kind was accepted; regression test failed as expected | Required nonblank source classification; reviewed the small guard and test, no further refactor justified |

Quality gates executed successfully on macOS/aarch64 with Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` — 21 integration tests passed, including table-driven invalid
  cache cases; no ignored tests.

Also inspected both help screens and the normal dependency graph. Source review
confirmed the only environment read is HOME in the binary, no production
`unwrap()`/`expect()` or unsafe code, and no HTTP client/runtime. CLI tests use
child-process `env_clear()` and isolated temporary directories; they do not
modify process-global environment. Fixture provenance and count semantics are
documented in `tests/fixtures/README.md` and `SPEC.md`.

Linux execution and CI remain unverified/deferred to 1d. No live account, real
API call, synchronization, persistence, or locking behavior was implemented or
claimed tested. The initial sandboxed dependency download could not resolve
crates.io; the authorized network-enabled Cargo run resolved dependencies, and
subsequent tests and quality gates succeeded locally.

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

Milestone 1d will add CI with equivalent checks on macOS and Linux and lockfile
enforcement:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
```

Use additional tooling only when it answers a concrete concern. Report which
checks actually ran and their results. The documentation stage had no Cargo
package; the completed 1a checks are recorded above.

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
