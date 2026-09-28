# Yomibu implementation plan

`SPEC.md` is the authoritative product and architecture baseline. This document
tracks delivery order and acceptance criteria; it does not authorize future
milestones merely by listing them.

## Current state and stopping point

Milestone **1d — Milestone acceptance is complete**. The macOS/Linux CI workflow
enforces the committed lockfile and repository toolchain. Public APIs, dependency
features, credentials, and errors have been reviewed; the durability error now
exposes its I/O cause. Quality gates passed locally on macOS/arm64 and in an
isolated Linux/arm64 container, then on GitHub-hosted Ubuntu/x86_64 and
macOS/arm64. A CI follow-up adds dependency caching and avoids redundant runs.
Stop after 1d; later product milestones remain unauthorized.

On 2026-09-27, the official Rust release page and `rustup update stable` both
confirmed Rust 1.98.1. Installed the exact 1.98.1 toolchain with rustfmt and Clippy,
and pinned it in `rust-toolchain.toml`, using edition 2024. The repository pin
selects Rust/Cargo 1.98.1 without changing the user's global 1.82.0 default.
`Cargo.lock` records the resolved dependencies. In 1b, reqwest 0.13.5, Tokio
1.53.1, and tempfile 3.27.0 support HTTP and persistence; wiremock 0.6.5 is a
development dependency. Proptest remains deferred.

## Vertical milestones

| Step | Status | Deliverable | Acceptance |
| --- | --- | --- | --- |
| 0 — Decisions | Complete | Three authoritative, consistent documents | Sourced external facts; proposed structure; Rust rationale; no application initialization |
| 1a — Offline status | Complete | Package, domain snapshot, cache reader, real `status` command | Fixture-backed integration tests; no token/network dependency; useful errors and summaries |
| 1b — First complete sync | Complete | HTTP adapter, normalization, safe persistence, thin CLI composition | Mock API → normalized snapshot → disk → offline status succeeds through real components |
| 1c — Sync resilience | Complete | Expanded failure-path verification and hardening of the 1b safety foundations | Failure cases preserve a usable complete cache; concurrent access and post-replacement errors behave as specified |
| 1d — Milestone acceptance | Complete | macOS/Linux CI and reviewed public library surface | Local and hosted macOS/Linux gates pass; documented limitations; no placeholder future features |

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

#### 1b delivery and TDD record

Behavior was developed in Red–Green–Refactor cycles. Each row records the observed
RED, the implemented GREEN, and explicit production/test refactor review followed
by a focused rerun. Dependency declarations, synthetic source fixtures, and test
module setup were test-enabling scaffolding.

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Private full replacement | `SyncGuard` absent, round-trip test did not compile | Added serialization, lock ownership, private directories/files, temporary-file flush/sync, atomic persist, and directory sync; reviewed ownership/error stages, no abstraction justified |
| Cache/account protection | A corrupt existing cache was overwritten | Validate existing caches and new snapshots; refuse account mismatch; refactored shared existing-cache loading and reran all cache tests; basic lock contention/read coexistence also passed |
| HTTP foundation | `Client` absent | Real bearer/revision requests normalize an empty profile; authentication, redirects, malformed JSON, and oversized responses fail safely; reviewed sanitized errors and bounds, no further change justified |
| Source normalization | Nonempty assignments returned `InvalidResponse` | Private typed DTOs normalize all three lexical variants, progress, and access exclusions; exact normalized mixed fixture matches; refactored progress iteration to avoid an intermediate allocation |
| Complete pagination/batching | Empty first pages lost progress; 101-ID request missed bounded mocks; unsafe-page test exposed a later-endpoint false positive and was tightened to fail specifically on pagination | Explicit termination, origin/path/credential checks, repeated-URL detection, and 100-ID batches passed; reviewed ordering and termination, no further refactor justified |
| Source integrity | Identical duplicates failed; invalid excluded levels/content and missing nullable fields were accepted | Collapse equal retained records, reject conflicts, require nullable source fields, validate content before exclusion; extracted shared subject validation and a small deduplication function for three collections |
| Retry foundation | Retry/reset helpers absent | Actual HTTP retries obey the two-retry budget and excessive resets fail; fixed-time tests verify 1/2-second backoffs and reset/default calculations; removed an unnecessary unreachable panic branch |
| CLI command/guidance | `sync` unrecognized; missing-cache text still said sync unavailable | Environment-only token handling, synchronous lock/write around async retrieval, and shared status output; reviewed branch-local environment/runtime use and reran CLI tests |
| Cross-component acceptance | Local-origin constructor private; constructor accepted unsafe remote HTTP | Validated library origin configuration; complete HTTP → cache → offline subprocess, repeat refresh/removal, and different-account preservation passed; reviewed the small public API and kept endpoint configuration out of CLI options |
| CLI composition | Extracted composition function absent | Tested the actual binary composition with mock HTTP, asserted the lock is held during requests, rendered and reloaded the same snapshot; reviewed runtime/guard lifetimes, no further refactor justified |

Quality gates passed on macOS/aarch64 with Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` — 39 tests passed (11 library HTTP tests, 1 binary composition
  test, 13 cache, 7 CLI, 6 summary, and 1 cross-component test); none ignored.
- `git diff --check`; reviewed new/untracked source and fixture files as well.

Also inspected CLI and sync help and the normal dependency graph. Production has
no `unwrap()`/`expect()`, unsafe code, credential-bearing error sources, DTO exports,
or token/config persistence. The only environment reads are HOME and the sync
branch's token read in the binary. Runtime startup, argument parsing, output, and
exit status remain there; the library owns HTTP, normalization, cache operations,
and summaries. Dependencies have only the required features enabled.

Sandboxed Cargo could not resolve crates.io or bind local listening ports. The
approved Cargo runs downloaded dependencies and ran loopback mocks successfully;
normal tests never contact the real WaniKani service. An attempted paused Tokio
clock advanced past real socket I/O, so retry delay calculations use a fixed input
clock and HTTP retry classification/budget tests use real loopback I/O (about six
seconds for bounded backoffs). No wall-clock timing thresholds are asserted.

At the 1b stopping point, no live account/API run, Linux execution, CI, expanded
streamed-body/deadline cases, deterministic fault injection around replacement/
directory sync, or cross-process interruption matrix had been verified. The
post-replacement error category existed without an induced directory-sync failure.
The 1c results below address the resilience gaps; Linux/CI remain in 1d.

### 1c — Sync resilience

- Expand verification and harden pagination, bounded retries, deadlines, streamed
  response limits, credential-safe URL handling, and structural integrity.
  Required safety foundations and representative tests were introduced in 1b.
- Extend the 1b reset/removal, permitted absence, access-exclusion, and contention
  cases to failures on later pages, cross-process contention, and interruptions.
- Verify old-cache preservation on every tested pre-replacement failure. Test
  post-replacement durability errors separately rather than asserting rollback.

#### 1c delivery and TDD record

The cancellation fix followed a confirmed behavioral RED before any production
change. Test-enabling private seams were introduced only after their focused
tests failed to compile. Coverage extensions for guarantees already implemented
in 1b passed without production changes. Each group received an explicit review
of production/test naming, duplication, modelling, ownership, and Rust idioms,
followed by focused reruns and the full gates.

| Cycle / verification group | Observed RED or existing behavior | GREEN and refactor review |
| --- | --- | --- |
| Cancelled rate-limit wait | Polling and dropping a fetch erased the pending deadline (`None` instead of the saved instant) | Clear the deadline only after the wait; focused regression passed; reviewed borrowing and cancellation, added the invariant comment, then reran |
| Storage fault boundaries | Tests could not compile without private checkpoints | Added one private generic callback around the existing write sequence; pre-replacement failures preserve bytes and parsed data, while directory failure exposes the complete new snapshot with `DurabilityUncertain`; consolidated the directory error mapping and staging-file lookup, then reran |
| HTTP deadlines | Focused stalled-response test could not compile without a private timeout constructor | Factored construction while keeping public defaults; 250 ms test deadlines cover headers and body and exhaust exactly three attempts; refactored shared cache-preservation assertions, then reran |
| Streamed bodies and retries | Existing 1b behavior passed the new cases | Exact 16 MiB chunked body succeeds; excess chunked/close-delimited bodies and declared oversize fail; truncated length/chunk framing exhaust retries; successful recovery discards partial bytes; shared the raw server's bounded response sequence and reviewed task cleanup |
| Rate timing, URL safety, and structural integrity | Existing guards passed expanded cases | Controlled time proves successful and 429 reset headers delay reused clients; fixed-clock reset boundaries, permanent errors, mixed transient budgets, hostile URLs, redirects, invalid excluded data, and broken references are covered; reviewed sanitized error chains and test-only helper visibility, no further abstraction justified |
| Later-page failures and changing state | Existing replacement rules passed the expanded matrix | Authentication, malformed JSON, server/rate errors, missing terminators, conflicts, and cycles on all three collections preserve cache bytes and offline output; access changes and removal of statistics/review-only content replace old state; reviewed fixture isolation and absence assertions, no production change justified |
| Process interruption and contention | Existing locking/replacement passed separate-process checks | Kill during fetch and at six write boundaries, including a partial staging file; verify complete old/new reads, private staging files, stable lock inode, release/reacquisition, and later replacement; real CLI contention fails while status works; reviewed readiness handshakes and RAII child cleanup, cleared child environments, and shared staging-file lookup, then reran |
| Actual rename failure | Existing error mapping passed a forced missing-source rename | The real persist operation fails after staging-file removal while old bytes and parsed data survive; reviewed cleanup and error classification, no further change justified |

The subsequent simplification pass merged overlapping HTTP integrity,
pagination, reset, and storage-fault matrices, removing 103 test lines while
retaining each distinct safety scenario. APIs and dependencies are unchanged.
Focused suites and full gates were rerun. The child helper now parks until killed;
both interruption tests require SIGKILL rather than accepting any failed exit.

Parallel validation exposed a lock-release edge case. A deterministic regression
with a duplicated lock-file handle failed with `Locked` after dropping its guard.
Explicit unlock in `SyncGuard::drop` made it pass; closing alone may retain the
lock through a briefly inherited descriptor during process creation. Refactor
review renamed `_lock` to `lock`, kept cleanup local to `Drop`, and verified that
dropping the old duplicate cannot unlock a subsequent guard. Focused storage
tests and all gates were rerun after that review.

Quality gates passed on macOS/arm64 with the pinned Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` — 56 test entries passed: 23 library, 1 binary composition,
  13 cache, 8 CLI, 6 summary, and 5 cross-component entries. Two entries are
  subprocess entry points; table-driven tests cover multiple failure scenarios.
  None ignored.
- `git diff --check`; reviewed the new test files as well as tracked diffs.

The first sandboxed baseline run could not bind loopback ports. Authorized test
runs used only local servers and synthetic credentials. No live API request was
made. Test processes receive their own environment overrides; no process-global
variables are mutated. Existing 1/2-second retry integration backoffs still use
real time, but rate-reset timing uses explicit clock advancement, with time
resumed before real socket I/O. Short deadline tests assert outcomes and request
counts, not elapsed wall-clock thresholds.

Fault injection is at storage operation boundaries, with a real rename-failure
case; it does not emulate failing hardware or power loss. Process-kill tests show
atomic visibility and lock recovery, not survival of a machine crash. Killed
writers may leave private staging files, which subsequent reads/writes ignore.
Connect-timeout configuration remains 10 seconds; DNS/TLS blackholes and live
service behavior are not separately simulated. Linux execution, CI, and final
public-surface acceptance remain in 1d. No deferred product features, new public
APIs, cache schema changes, or new crates were added.

Rust notes for a Ruby developer: a dropped async future stops at an `await`, so
state needed by the next call must remain owned by the client until that wait
completes. `impl FnMut` provides a small private, statically dispatched test seam
without a public adapter hierarchy. `Drop` releases normal-scope resources, while
process-kill tests separately verify the OS's file-lock cleanup when destructors
do not run.

### 1d — Milestone acceptance

- Run the quality gates on macOS and Linux and enforce the lockfile in CI.
- Review the public library surface, dependency features, secret handling, error
  messages, and alignment with `SPEC.md`.
- Document actual validation results and remaining limitations. Do not make a live
  account or a real API request a prerequisite for the automated test suite.

#### 1d delivery and TDD record

Added `.github/workflows/ci.yml`: push and pull-request events run one matrix on
`ubuntu-latest` and `macos-latest`, with independent results and a 20-minute job
deadline. Rustup reads the exact version and components from
`rust-toolchain.toml`; there is no second toolchain pin. Each job verifies that
`Cargo.lock` is tracked, runs formatting, Clippy, and tests with lockfile
enforcement, then checks that the lockfile is unchanged. Checkout is pinned to
the verified v7.0.1 commit, with read-only contents permission and credential
persistence disabled. CI requires no WaniKani credential or live account.
The initial workflow introduced no extra build scripts, caching layer, or testing
infrastructure. The caching follow-up is recorded below.

Reviewed the workflow against the official
[checkout documentation](https://github.com/actions/checkout/tree/v7.0.1),
[workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax),
and [Rustup toolchain-file behavior](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file).
The Linux run also confirmed that `rustup show active-toolchain` installs the
file's missing rustfmt/Clippy components.

| Review area | Finding and disposition |
| --- | --- |
| Public library surface | Retained the four concrete modules and existing signatures. Added rustdoc for validated public data, borrowed summaries, lock ownership, trusted custom origins, Tokio requirements, client reuse/cancellation, and replacement outcomes. Transport DTOs and fault/timeout seams remain private. |
| Dependency features | Inspected normal/build and feature graphs: reqwest uses Rustls with defaults off; clap and chrono retain their narrow feature sets; Tokio directly enables `rt`, `time`, and `net`, with `io-util`/`sync` also required transitively. Development features support existing HTTP/clock tests. No `native-tls`, unnecessary direct feature, new crate, or lockfile change was found or introduced. |
| Secret handling | The binary alone reads HOME and the sync token; status still needs neither credentials nor an HTTP client. Authorization is marked sensitive, the client has no Debug/Serialize implementation, and HTTP errors discard URLs/bodies/transport causes. Existing hostile-URL, redirect, malformed-body, error-chain, and CLI tests passed with synthetic tokens. No credential/config persistence was added. |
| Errors and recovery | Existing messages distinguish missing/corrupt/unsupported caches, authentication, lock contention, account mismatch, pre-replacement failures, and uncertain durability. Found and fixed the missing I/O cause on `DurabilityUncertain`, retaining its variant and message. |
| Specification and scope | Cache schema, source-state semantics, CLI options, HTTP limits, and synchronous/async boundaries remain unchanged. No future product feature or abstraction was added. |

The only behavioral change followed strict Red–Green–Refactor:

- **RED:** extended the existing storage-fault matrix to inspect the underlying
  `io::Error` and its kind. The focused test failed specifically at
  `SyncDirectory` with “missing I/O cause”; earlier fault boundaries passed.
- **GREEN:** marked the existing `DurabilityUncertain` field with `#[source]`.
  The same focused test passed, including actual rename failure and old/new
  cache preservation assertions.
- **REFACTOR:** reviewed production and test naming, modelling, duplication,
  ownership/borrowing, and Rust idioms. Reused the fault matrix and private seam;
  no additional code abstraction or behavior change was justified. Placed API
  documentation before derives and reran the focused test successfully.

CI and documentation changes are non-behavioral scaffolding and did not receive
artificial failing tests.

Validation executed on 2026-09-28 with pinned Rust/Cargo 1.98.1:

| Environment | Formatting | Clippy | Tests |
| --- | --- | --- | --- |
| Native macOS/arm64 | Passed | Passed, warnings denied | 56 entries passed, none ignored |
| Linux/aarch64, Debian 12 container under OrbStack | Passed | Passed, warnings denied | 56 entries passed, none ignored |

Both environments ran `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`, and
`cargo test --locked --all`. The 56 entries comprise 23 library, 1 binary, 13
cache, 8 CLI, 6 summary, and 5 cross-component tests, including two subprocess
entry points. Linux used the official `rust:1.98.1-slim-bookworm` image, copying
source from a read-only mount into its own filesystem; the final lockfile matched
the repository byte for byte. Native tests used authorized loopback networking.

Additional checks passed:

- Actionlint 1.7.12, downloaded to a temporary directory from its official release
  and checked against the release checksum, reported no workflow errors.
- Temporary manifest copies with missing and stale lockfiles each failed Clippy
  with exit 101 specifically because of `--locked`; neither lockfile was created
  or changed. The repository manifest and lockfile were untouched.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` built the library
  documentation without warnings; inspected top-level, sync, and status help.
- Reviewed production code for public exports, environment reads, dynamic errors,
  unsafe code, and recoverable `unwrap()`/`expect()`; no additional findings.
- `git diff --check`, review of the new workflow, and unchanged manifest,
  toolchain, and lockfile checks passed.

At initial delivery, hosted runs were unverified; the subsequent successful
Ubuntu/x86_64 and macOS/arm64 run is recorded below. No live WaniKani request,
DNS/TLS-blackhole simulation, hardware-fault test, or power-loss
durability test was performed. Existing process-kill/fault-injection limitations
from 1c still apply. Per-request deadlines and page-size bounds do not impose a
total refresh deadline or collection-size bound; this existing API limit is now
documented. No additional platform support is claimed.

Rust notes for a Ruby developer: formatting an inner error in `Display` does not
automatically expose it through `std::error::Error::source()`. `#[source]` makes
the cause inspectable while keeping the meaningful outer enum variant. Cargo
features are additive across dependencies, so the resolved feature graph matters
as well as each direct dependency declaration. `--locked` refuses resolution
changes rather than silently editing the dependency snapshot.

#### CI caching and scheduling follow-up

The [first hosted run](https://github.com/kalorz/yomibu/actions/runs/36390858621)
passed every gate and all 56 test entries on both platforms. It took 3m 10s
overall: Linux ran for 1m 54s and macOS for 3m 02s, starting four seconds apart.
Actual test execution took about 19–21 seconds per OS; dependency checking and
compilation dominated the run. No cache was restored or saved in that workflow.

The follow-up pins `Swatinem/rust-cache` v2.9.2 to its verified release commit,
after toolchain selection and the tracked-lockfile check. It reuses dependency
downloads and compiled dependencies with the action's platform/compiler/manifest
keys. Only `main` saves caches; PRs can restore the base branch's caches. Cold
builds still run all gates. The Rust toolchain itself is not cached by this step.
See the [action's cache contract](https://github.com/Swatinem/rust-cache/tree/v2.9.2).

Push events are restricted to `main`, while pull-request events cover proposed
changes, avoiding duplicate branch-push/PR runs. Workflow-level concurrency
cancels superseded runs for the same ref without serializing the OS matrix.
Documentation-only skipping remains deferred; both OS jobs retain every gate.
These CI/documentation edits are exempt from artificial behavioral RED tests.
Review kept the existing matrix and commands without a new script or helper.

Validation on 2026-09-28: actionlint 1.7.12 and `git diff --check` passed.
The [cold attempt](https://github.com/kalorz/yomibu/actions/runs/36393246817/attempts/1)
and [warm rerun](https://github.com/kalorz/yomibu/actions/runs/36393246817/attempts/2)
of commit `04db3a7` both passed formatting, locked Clippy with warnings denied,
all 56 test entries per OS, and unchanged-lockfile checks. The warm logs confirm
exact cache hits with distinct Linux/x64 and Darwin/arm64 keys.

| Run | Linux job | macOS job | Overall elapsed |
| --- | --- | --- | --- |
| Original uncached workflow | 1m 54s | 3m 02s | 3m 10s |
| New workflow, populating caches | 1m 35s | 2m 34s | 2m 41s |
| Same commit, warm caches | 45s | 54s | 1m 04s |

Warm Clippy took about 2s/4s and test compilation 6s/8s on Linux/macOS;
the full tests still executed. These are individual measurements, not a runtime
guarantee: queueing, runner changes, dependency/toolchain changes, and cache
eviction affect later runs. PR scheduling and cancellation were linted/reviewed,
without creating a synthetic PR or deliberately overlapping runs. No Rust source,
dependencies, lockfile, or test coverage changed.

Rust note for a Ruby developer: this cache retains compiled dependency artifacts
as well as downloads. Their compatibility depends on the compiler and target
platform, so Linux and macOS need separate cache entries.

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

Milestone 1d adds CI with equivalent checks on macOS and Linux and lockfile
enforcement; use the same locked commands locally:

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
