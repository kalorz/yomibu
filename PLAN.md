# Yomibu implementation plan

`SPEC.md` is the authoritative product and architecture baseline. This document
tracks delivery order and acceptance criteria; it does not authorize future
milestones merely by listing them. `ARCHITECTURE.md` records responsibilities,
composition, and file/package/repository boundaries.

## Current state and stopping point

Milestone **1d — Milestone acceptance is complete**. The macOS/Linux CI workflow
enforces the committed lockfile and repository toolchain. Public APIs, dependency
features, credentials, and errors have been reviewed; the durability error now
exposes its I/O cause. Quality gates passed locally on macOS/arm64 and in an
isolated Linux/arm64 container, then on GitHub-hosted Ubuntu/x86_64 and
macOS/arm64. Follow-ups add dependency caching, avoid redundant runs, and overlap
independent HTTP test scenarios while strengthening exact timer checks.
On 2026-10-01 the user authorized the architecture migration for existing
`sync/status` described in `ARCHITECTURE.md`. That migration is complete; its
native macOS verification is recorded below separately from historical CI runs.
Generation and other later product milestones remain outside this scope.

**G0 — Manual candidate preview is complete** in the subsequent 2026-10-01
Cloud implementation. The synchronous public library operation, thin CLI,
independent checks, and runnable direct-library example are delivered below.
The original six TDD cycles, review follow-ups, and verification are recorded in
the G0 section.
The next offline learner-constraints/retrieval slice was approved on 2026-10-02
and is in progress below; generation remains unimplemented.

The 2026-09-30 naming follow-up adopts the design vocabulary in `SPEC.md` and
renames the existing sync-data type. It does not start a new product milestone.

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
| Architecture migration | Complete | Library `App`, explicit source/storage contracts, file and in-memory stores, adapter layout, architecture document | Both stores run real sync/status; existing safety and schema contracts preserved; no generation placeholders |
| G0 — Manual candidate preview | Complete | Synchronous library operation and `preview` CLI, structured word entries, deterministic selection and independent checks | Direct and CLI calls agree, no storage/network/runtime requirements, explicit assessment limits; all 85 tests and required gates pass on Linux/x86_64 |

Implementation steps use small Red-Green-Refactor cycles (see `AGENTS.md`). Tests
accompany behavior, beginning with a confirmed failing test, rather than being
added after implementation or postponed to 1c.

Basic credential protection, timeouts, and safe persistence apply as soon as the
respective I/O is introduced; 1c completes and exercises the failure paths rather
than retrofitting unsafe foundations.

## G0 — Manual candidate preview

This delivered slice precedes learner-source composition, Japanese analysis,
and LLM generation. It implements `SPEC.md`'s **Manual candidate preview**
contract and the minimal adaptation described in `ARCHITECTURE.md`.

### Outcome

```sh
yomibu preview \
  --word '猫:ねこ:cat' \
  --word '犬:いぬ:dog' \
  --word '学校:がっこう:school' \
  --grammar 'です' --grammar 'は' \
  --take 2
```

Returns the first two structured word entries and check results. Grammar and
linguistic correctness are explicitly unassessed. This is an executable preview,
not a validated Japanese exercise. The same behavior is callable from Rust with
structured values, without a store or CLI process.

### Implementation sequence and acceptance

Work through these in small confirmed Red-Green-Refactor cycles. Record the
observed RED, GREEN, and explicit refactor review as work completes; do not write
the full implementation before its tests.

1. **Structured input and deterministic selection.** Validate required word
   fields and grammar descriptions, require a positive count within input size,
   and retain input order and associations of text/reading/meaning. Cover empty
   input, invalid counts, duplicates, and the same spelling with different
   reading/meaning entries. Do not infer lexical equivalence or mastery.
2. **Independent result checks and reporting.** Check the produced selection
   against supplied entries and the requested count. Exercise production check
   logic with a valid selection and deliberately invalid data, including an
   unsupplied reading/meaning combination and a wrong count. Preserve grammar
   inputs and report them as unassessed. No tokenizer, model, or fake linguistic
   assessment is involved. Export only types needed by actual library callers.
3. **Thin CLI composition.** Add repeatable `--word 'TEXT:READING:MEANING'`,
   repeatable `--grammar`, and required `--take`. Split at the first two colons,
   reject missing/blank fields, preserve colons inside meanings, and show useful
   errors/help. Render the selected entries and assessment limits. Keep syntax
   parsing outside the library and direct-call behavior equivalent.
4. **Isolation and regression verification.** Run preview with no HOME or token
   in an isolated child-process environment and confirm it creates no data
   directory/cache. Move data-dir resolution into storage-dependent CLI paths;
   do not change global environment in parallel tests. Existing sync/status,
   account isolation, locking, and durability semantics must remain covered.
5. **Review and delivery.** Review naming, ownership, public surface, synchronous
   flow, and whether any trait is justified by actual substitution. No separate
   architecture rewrite, adapter migration, workspace split, or generic plugin
   host. Run the gates below, demonstrate the CLI and direct library use, update
   this section with actual evidence and limitations, and open a reviewable PR.

Definition of done:

- [x] The example selects exactly `猫:ねこ:cat` and `犬:いぬ:dog` in order.
- [x] Required fields, delimiters, counts, duplicates, and meaning/reading
  association have behavioral coverage; invalid inputs never report success.
- [x] Result checks can detect invalid candidate data independently of selection.
- [x] Grammar inputs survive preparation and remain explicitly unassessed.
- [x] Direct library use is synchronous and needs no CLI/runtime/account/store.
- [x] The CLI works without HOME, credentials, cache, or filesystem writes.
- [x] Existing sync/status and persistence regression tests pass.
- [x] `cargo fmt --check`, locked Clippy with all targets/features and warnings
  denied, and `cargo test --locked --all` pass; report any platform limitations.
- [x] The final diff and public API are reviewed; TDD/refactor evidence is
  recorded; documentation reflects implemented behavior; changes are committed
  and available in a pull request. Do not mark this done at planning time.

### G0 implementation TDD record — 2026-10-01

Started from latest merged `main`, `96b3eda`, after confirming the G0 contract.
Each row records a confirmed RED before production code, GREEN, and an explicit
refactor review followed by a focused rerun.

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Structured selection and counts | Integration tests could not import the absent `preview` API | Select a borrowed prefix, preserving duplicates and associated fields; reject zero, excessive counts, and empty input. Three tests passed. Reviewed names, ownership, and test cases; borrowing avoids cloning strings, with no further abstraction justified. |
| Required word fields | A blank text field was accepted | Validate every supplied entry, including the unselected suffix, for blank text/reading/meaning; return an entry-specific typed error. Four tests passed, including Unicode whitespace cases. Reviewed the compact field loop and test matrix; no additional type/helper or refactor was justified. |
| Grammar declarations | The result lacked grammar inputs; after retention was added, a blank description was still accepted | Preserve descriptions, order, duplicates, and optional empty grammar input; reject blank descriptions with a typed error. Six tests passed. Reviewed borrowing and validation: share the input lifetime, retain description content verbatim, and avoid a grammar identity/adapter type; no further refactor justified. |
| Independent checks | Check function/outcome types were absent | Production checks accept supplied entries, reject unsupplied/recombined text-reading-meaning entries, and detect empty/excess selections independently of membership. Public results report grammar and linguistic correctness as unassessed. Three checker tests and six direct API tests passed. Refactored input validation into a private function so validation, selection, and checking are explicit; kept the checker private with no trait or generator seam. Focused suites passed again. |
| Thin CLI and syntax | The example/error cases failed on the unrecognized `preview` command; delimiter tests could not find the parser | Added executable-only parsing/rendering around the library call. Two parser tests cover ASCII delimiters, boundary trimming, internal whitespace, and meaning colons; two CLI tests compare the documented example to direct library use and cover eleven invalid-input cases. Reviewed ownership, error boundaries, and rendering duplication; shared library field validation and one outcome-rendering loop suffice, with no further abstraction justified. Focused tests passed after formatting/review. |
| Preview isolation | Preview failed with “HOME is unavailable” in an environment-cleared child process | Resolve data directories only in sync/status through a shared private helper. Preview succeeds with no HOME/token, an unusable or absent explicit directory, and an invalid token; isolated directories and sentinel bytes remain unchanged. All twelve CLI tests passed. Refactor review preserved sync/status error ordering and kept runtime/source construction in sync; no further change justified. The complete CLI suite passed again. |

### G0 verification and review

Executed on Linux/x86_64 with the repository's exact Rust 1.98.1 toolchain:

- `cargo fmt --check` — passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `cargo test --locked --all` — 84 test entries passed, none ignored: 28 library,
  3 binary, 5 App, 13 cache, 12 CLI, 6 preview, 6 store, 6 summary, and 5 sync.
  This includes the two existing subprocess helpers. The existing rustdoc
  example also compiled successfully.
- Ran the documented command above through `env -i` with no HOME/token. Output:

  ```text
  Manual candidate preview (not a validated Japanese exercise)
    Word: 猫:ねこ:cat
    Word: 犬:いぬ:dog
    Grammar: です
    Grammar: は
  Supplied-entry membership: pass
  Requested entry count: pass
  Grammar: not assessed
  Readings, meanings, naturalness: not assessed
  ```

- `cargo run --locked --example preview` — passed; the synchronous example
  asserts the same selected entries, retained grammar, and check outcomes, then
  prints the structured result. The compiled example also passed under `env -i`.
- Inspected top-level and preview help. `git diff --check` passed. The complete
  diff, including new source/tests/example files, was reviewed for scope,
  ownership, public API, errors, and synchronous data flow.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` — passed, including
  the new public API links.

Final refactor review kept one cohesive preview module and five public data/error
types needed by callers. Input validation and result checking remain private;
no substitution need justifies a new trait. The result borrows immutable inputs,
so words/grammar are not cloned. The CLI alone parses delimiters, reads the
environment for storage commands, renders output, and selects exit status.
Manifest, lockfile, toolchain pin, source/store contracts, persistence code,
schema-1 fixtures, and existing tests are unchanged. Their full regression suite
passed, including account isolation, writer locking, failure preservation, and
uncertain durability.

The Cloud environment initially had no Rust installation. Installed the pinned
toolchain and downloaded the locked dependencies into the workspace; no
repository dependency/toolchain changes were required. Network-enabled execution
was needed for fetching/build setup and the full loopback HTTP test suite.
Preview demonstrations themselves used no network or credentials.

Hosted verification of implementation commit `f77809a` also passed on both
Ubuntu and macOS in [CI run 36921406965](https://github.com/kalorz/yomibu/actions/runs/36921406965).
Both jobs completed formatting, locked Clippy, all tests, and unchanged-lockfile
checks successfully. This is G0 evidence; earlier hosted results elsewhere in
this document remain historical. The follow-up recording this run changes only
this document.

Limitations: local execution was on Linux/x86_64; macOS was verified through
GitHub-hosted CI. No live WaniKani or model call was made. Membership proves only
exact supplied-entry association, not dictionary truth, grammar usage,
naturalness, or Japanese correctness. No later milestone started.

Rust notes for a Ruby developer: `Preview<'a>` borrows slices, so the compiler
prevents changing or dropping the inputs while the result still refers to them.
`Result<_, PreviewError>` and enum outcomes keep input failures and unassessed
checks explicit instead of relying on exceptions or truthy values. An ordinary
synchronous function is sufficient here; no service object, runtime, or new
trait is needed.

### G0 review follow-up — rendering and membership lookup

Addressed the two findings in Greptile's [PR #3 review](https://github.com/kalorz/yomibu/pull/3#issuecomment-5939929148).

- **Rendering RED:** a new subprocess regression failed because embedded newlines
  created extra word/assessment lines and terminal controls were emitted raw.
  **GREEN:** apply `str::escape_debug` when rendering each word field and grammar
  description. The test covers all three word fields, grammar, LF/CR/tab, terminal
  escape sequences, Unicode line/paragraph separators, and literal backslashes.
  The library inputs remain unchanged. **REFACTOR:** reviewed production/test
  code and kept the standard borrowed display iterator, avoiding allocated
  replacement strings, input restrictions, or a custom escaping helper. No
  further refactor was justified; all thirteen CLI tests passed after review.
- **Membership refactor:** the supplied-prefix lookup made N(N+1)/2 comparisons
  for N distinct selected entries. Existing independent checker and public API
  tests passed before changing this behavior-preserving implementation. Replaced
  repeated slice scans with one borrowed `HashSet`; `WordEntry` derives `Hash`
  alongside complete-entry equality. The result still comes from the original
  slice, preserving order and duplicates. No new type, dependency, or adapter was
  introduced. This is a performance refactor, not a new behavioral contract;
  no artificial failing behavior test or timing assertion was added. Reviewed
  ownership, exact-entry matching, hashing cost, and auxiliary memory. All three
  checker tests and six public preview tests passed again after that review.

A temporary direct-library probe measured the median of three calls per size,
with distinct, fixed-width text entries and `take` equal to input size. Input
construction and output were outside the timed calls. The same debug-build probe
was linked against the library before and after the refactor:

| Entries | Slice scans | Borrowed hash set |
| --- | --- | --- |
| 1,000 | 7.590 ms | 1.367 ms |
| 2,000 | 30.242 ms | 2.716 ms |
| 4,000 | 120.492 ms | 5.431 ms |
| 8,000 | 456.895 ms | 11.184 ms |

These are local Linux/x86_64 diagnostic measurements, not release benchmarks,
CI thresholds, or a worst-case timing guarantee. The new check trades O(n)
borrowed-entry storage and string hashing for expected O(n + k) entry operations;
hash collisions still use exact equality. No production input limit was added.

Follow-up verification on Linux/x86_64 with pinned Rust 1.98.1 passed:

- `cargo fmt --check`.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `cargo test --locked --all` — 85 test entries passed, none ignored, plus the
  existing rustdoc example. The only added test is the rendering regression;
  existing source, storage, sync/status, and preview coverage remains intact.
- Reviewed the complete follow-up diff and public API; `git diff --check` passed.
  All three design documents reflect the fixes. Dependencies, toolchain pin,
  schema-1 persistence, and sync/status implementation remain unchanged.

Rust note for a Ruby developer: `HashSet<&WordEntry>` stores references and uses
the entry's derived value hash/equality, without copying its strings. It supports
membership lookup; it does not supply output ordering or remove output duplicates.
`escape_debug()` is a display iterator, so escaping does not modify source values
or require allocating replacement strings.

### Scope limits and next decisions

G0 does not add file/stdin/JSON imports, real Japanese generation or grammar
matching, model calls, provider integrations, storage schema changes, or future
text/plugin types without an executable use. Existing concepts in the target
architecture are guidance, not a checklist of types to implement.

After G0, continue with learner constraints/retrieval and an evidence-backed
Japanese analysis slice, then validated generation. Before claiming linguistic
coverage, select pinned tools/data, review applicable licenses, and evaluate
representative known/unknown and ambiguous cases. Typed model-task composition,
global budgets, basic naturalness/coherence review, and complete post-repair
revalidation arrive with real generation. Broader translation judging and
adaptive selection remain later milestones. No real-model budget is authorized
by G0.

### Original Cloud handoff (planning record)

Merge the documentation-only planning PR before starting from `main`. Otherwise,
explicitly select its branch, `codex/generation-preview-plan`, as the starting
point. A local saved file or commit alone is not the remote handoff. Confirm this
G0 section exists in the selected checkout before implementation; do not recreate
decisions from the old shared-chat transcript. The three repository documents
contain the consolidated direction and are sufficient task context.

Suggested implementation prompt:

```text
Work in kalorz/yomibu from the latest main containing "G0 — Manual candidate
preview" in PLAN.md. Read AGENTS.md, SPEC.md, ARCHITECTURE.md, and PLAN.md, then
inspect the actual implementation before editing. If the G0 plan is absent,
report the base-branch mismatch rather than inventing the missing requirements.

Implement only G0 end to end on a new codex/ branch: a synchronous public library
preview and the thin CLI command described in SPEC.md. Preserve structured
text/reading/meaning associations, grammar declarations, deterministic selection,
independent membership/count checks, typed errors, and honest unassessed statuses.
Preview must work without HOME, tokens, a store, network access, or an async
runtime. Keep existing sync/status behavior and schema-1 persistence intact.

Use strict Red-Green-Refactor, confirming each focused RED before production
behavior and explicitly reviewing/refactoring after GREEN. Keep the public API
small and adapt existing code incrementally; do not scaffold the future text
hierarchy, language analysis, model integration, or a general plugin framework.
Proceed autonomously on routine reversible choices; do not stop after a plan.

Run cargo fmt --check, cargo clippy --locked --all-targets --all-features -- -D
warnings, and cargo test --locked --all. Demonstrate the example command and
direct library use. Update PLAN.md with actual TDD/refactor evidence, check
results, and limitations; keep the specification and architecture aligned.
Review the complete diff, commit and push the implementation, and open a PR.
Report the PR link, commit, verification, and brief Rust notes for a Ruby
developer. Do not merge automatically or start the following milestone.
```

### Planning verification — 2026-10-01

Prepared from merged `main` at `b2b50e8`. Reviewed the three-document diff for
consistent scope, vocabulary, CLI contracts, and acceptance criteria; checked
whitespace with `git diff --check`. Only SPEC.md, ARCHITECTURE.md, and PLAN.md are
changed. No Rust source, tests, dependencies, or CI configuration are changed,
and no Cargo gates or linguistic/model experiments were run for this
documentation-only handoff. At that point G0 remained unimplemented; its delivery
and verification are now recorded above.

## Learner constraints and retrieval — approved preparation slice

Approved on 2026-10-02: implement only offline practice-context preparation from
preserved WaniKani progress, local manual grammar assertions, a revisable concrete
knowledge policy, and explicit word/reading/sense targets. SPEC.md defines the
behavior and ARCHITECTURE.md the composition boundaries.

The default uses a recorded lesson start; recorded pass is an explicit alternative.
Both exclude unavailable/hidden material and require an assignment. Grammar
descriptions assert familiarity; technical IDs are local to the loaded input.
Lexical retrieval uses only cached WaniKani data, with exact accepted fields and
explicit unassessed reading/sense association and example suitability.

Implementation starts from main `22b7c92` on
`codex/learner-context-preparation`. PR #3 and both Greptile fixes were verified
in main. The authoritative G0 documents agree with the code; README omits G0 and
will be brought up to date.

The cloud executor failed provisioning and remains unavailable. No local work
was inspected or changed. Work uses GitHub commits on the separate branch and
the existing Ubuntu/macOS PR CI for observed Red–Green–Refactor execution. No
local execution, private learner cache, or real-data demonstration is claimed.

Acceptance:
- [ ] Grammar input preserves assertions/duplicates/identity and rejects invalid input.
- [ ] Policy decisions are explainable and recomputable without mutating source data.
- [ ] Targets preserve complete tuples and cannot override learner eligibility.
- [ ] Retrieval has explicit missing/ambiguous/unsupported outcomes and provenance.
- [ ] CLI/direct calls agree; no implicit sync, runtime, credentials, or writes.
- [ ] Runnable synthetic demonstrations and assessment limits are documented.
- [ ] Existing G0, sync/status, schema-1 and durability regressions pass.
- [ ] Required locked quality gates pass on the final implementation.
- [ ] Full diff/public API review, commit/push, and PR delivery are complete.

Develop one behavior at a time: observed RED, minimal GREEN, explicit refactor
review, and focused/full reruns. Record actual evidence below; do not write
production behavior before its failing test. Real learner data remains a separate
unverified demonstration unless an authorized private cache becomes available.

### Implementation evidence

Initial grammar cycles are recorded in commit history. Confirmed REDs: missing
`grammar` module (`db700412`, run 36994881967), blank assertion accepted
(`99291d6`, run 36995270179), and missing file adapter (`4896ebb`, run 36995571545).
Each received a minimal GREEN, followed by explicit ownership/error/API review.
The preservation test passed again in the blank-validation RED. The file-input
refactor removed an unnecessary dummy read; Ubuntu CI rerun 36996003851 passed.
Knowledge derivation RED was confirmed at `9d3ed01` (run 36996345316), then both
platforms passed at `337a0f7` (run 36996845899). The explicit refactor consolidated
timestamp decisions; its Ubuntu rerun 36997118549 passed. Retrieval RED was
confirmed at `0ef5ca6` (run 36997244224); both platforms passed at `1377265`
(run 36997692088). Retrieval refactor review covered borrowing, indexing, exact
fields, ambiguity, errors and public surface; no further change was justified.
CLI tests now exercise the next behavior and rerun existing domain suites.

## Completed milestone records

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

#### Test execution follow-up

The slow tests spent most of their time in real retry waits of one and two seconds.
Three independent endpoint matrices serialized those waits, as did pairs of stalled
and truncated response scenarios. The follow-up uses existing Tokio `join!` and
local async closures to overlap each group, with at most three independent
servers/caches in a group. The successful HTTP retry and exhausted retry cases
are now separate tests. All 21 later-page failure cases retain their request-count,
error, credential, cache-byte, parsed-state, offline-status, and lock assertions.
Real socket tests retain their real timers, stalled requests, and retry budgets.

For exact transient timing, a socket-free paused-clock test replaces the two
duration-value assertions. It verifies that each wait is pending immediately
before its deadline (one or two seconds) and completes at the deadline. The existing
private duration helper now owns the existing sleep; production retry behavior stays the
same. No test-only delay configuration, dependency, public API, or runner was added.
Paused clocks are not used while real socket I/O is pending.

- **RED:** added the focused timer test first; it failed to compile because the
  private wait helper did not exist.
- **GREEN:** moved the existing sleep into that helper; the focused test passed
  in 0.00s.
- **REFACTOR:** reviewed naming, duplication, ownership, case isolation, cleanup,
  and concurrency bounds. Local closures reuse each matrix's assertions; split
  retry tests distinguish recovery from exhaustion, and the reset test now names
  its specific responsibility. No further production abstraction was justified.
  The focused HTTP suite (21 entries) and later-page matrix passed after review.
  Test scheduling changes are infrastructure refactors and need no artificial
  behavioral failure.

Validation on 2026-09-28: native macOS/arm64 and a disposable Debian 12
Linux/arm64 container passed `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`, and
`cargo test --locked --all`. All 58 entries passed, none ignored: 25 library,
1 binary, 13 cache, 8 CLI, 6 summary, and 5 cross-component entries, including
the same two subprocess helpers. The increase comes from one new timer test
and splitting an existing test; no scenario was removed. `git diff --check`
also passed. The container used the pinned Rust 1.98.1 image and a read-only
source mount, and its lockfile remained byte-identical to the committed file.

| Native macOS execution | Before | After |
| --- | --- | --- |
| Library tests | 7.57s | 3.80s |
| Cross-component tests | 9.55s | 3.85s |
| All test binaries, summed reported execution | 17.84s | 8.10s |

The Linux container reported 4.23s for the library and 3.88s for cross-component
tests, or 8.31s summed across all test binaries after the change.

The [hosted run for `8757ade`](https://github.com/kalorz/yomibu/actions/runs/36445060921)
passed every gate and all 58 entries on Ubuntu/x86_64 and macOS/arm64. Both jobs
restored exact dependency-cache hits. Summed test execution was 10.54s on Linux
and 10.64s on macOS, excluding test compilation of 6.37s and 12.15s respectively.
Clippy took 2.16s/5.61s; toolchain setup and cache restoration still contributed
about 12s/16s. The OS jobs overlapped.

| Hosted run | Linux job | macOS job | Overall elapsed |
| --- | --- | --- | --- |
| [Previous commit, `6f79d1b`](https://github.com/kalorz/yomibu/actions/runs/36393872248) | 46s | 1m 09s | 1m 19s |
| Test follow-up, `8757ade` | 36s | 55s | 1m 06s |

The follow-up did not achieve a workflow below 45s or 30s. An earlier warm run
already finished in 1m 04s, illustrating the setup/runner variation even though
test execution is now roughly halved. No fixed hosted runtime is promised.

These single-run measurements exclude compilation and process startup; they are
not timing assertions or a CI runtime guarantee. Real backoffs remain a floor
for HTTP tests, and hosted setup, cache restoration, compilation, and scheduling
still take time. Existing live-service, DNS/TLS, and power-loss verification
limits remain. Product work stops at milestone 1d.

Rust note for a Ruby developer: `join!` polls independent futures on the same
runtime, so one case can progress while another waits. Each future owns its
server and temporary directory; unwinding still drops those resources. This
reduces serialized waiting without weakening the real HTTP boundary checks.

## Naming and design follow-up — 2026-09-30

Accepted the responsibility vocabulary in `SPEC.md`, including
`SourceConnection`, `LearnerProgress`, `LearnerKnowledge`,
`LearnerKnowledgePolicy`, `PromptTemplate`, `ModelRequest`, `PromptStore`,
`CandidateGenerator`, and `ExerciseGenerator`. Future adapters use the
`InMemory...` prefix. These names define a direction, not a list of scaffolding
to add. Grammar entries retain provider identity; no cross-provider grammar
ontology is required. Code may remain public while Cloud supplies private data.

Renamed the implemented `domain::Snapshot` to `domain::WaniKaniSyncData`, matching
its account scope and synchronization interval. Renamed the validation-error
variants from `InvalidSnapshot` to `InvalidSyncData`, updated callers, local
variables, test names, and documentation. This changes Rust source API names and
diagnostic wording; it does not change validation, network requests, or storage
behavior. No compatibility alias is retained in this unpublished library.
The private persistence envelope keeps its `snapshot` field and schema version 1;
existing JSON fixtures remain unchanged.

The future composition retains explicit dependencies and optional sync. Direct
in-memory data is enough for one-off generation, local CLI storage remains
file-backed, and Cloud selects its stores. `LearnerKnowledge` is derived on
demand. Separate writes from reads without adding event sourcing, a command bus,
or a separate read database. No future types, traits, adapters, or crates were
implemented in this follow-up.

This is a naming refactor and documentation update, with no new behavioral path
requiring an artificial failing test. Before the rename, all 19 existing cache
and summary integration tests passed.

Refactor review covered production and test naming, responsibilities, duplication,
ownership/borrowing, and Rust idioms. Kept the existing modules, owned sync data,
borrowed summaries, private persistence envelope, and behavioral assertions;
no further abstraction or refactor was justified. One missed test-variable rename
was found by compilation and corrected before the final checks.

Post-change verification on native macOS passed:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all` — all 58 entries passed, including the two
  subprocess helpers; none ignored. The first sandboxed attempt could not bind
  loopback sockets; the authorized rerun passed with local test servers.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`
- `git diff --check` and review of the complete diff; schema fixtures, dependency
  manifest, lockfile, and toolchain pin remain unchanged.

Existing fixture reads, writer round trips, and HTTP-to-cache-to-CLI tests verify
schema-1 compatibility. No live API, model, Linux, or hosted CI run was performed
for this naming follow-up; earlier platform results remain historical evidence.

Rust note for a Ruby developer: renaming a Rust struct does not rename its JSON
fields. The private cache envelope preserves the disk format independently of
the public type name. Public source code also does not require every Rust type
or test helper to be exported with `pub`.

## Architecture migration — 2026-10-01

Implemented the accepted first slice: existing sync/status behind library `App`,
with explicit source and store dependencies. `App` has no runtime/environment
ownership or current-user state. Status needs no source; sync returns an owned
summary and explicit volatile/durable persistence outcome, preserving typed
source and storage errors. The CLI now composes these use cases instead of
implementing synchronization itself.

Moved WaniKani under `adapters/sources/wanikani` and file persistence under
`adapters/stores/file`. Root `wanikani` and `cache` module paths remain re-exports.
Added `InMemoryLearningStore`, which shares immutable versions only across
explicitly cloned handles, validates account/data invariants, and reserves a
writer across retrieval without holding a data lock during network I/O.
The physical `LearningStore` publishes related material and progress together.
Separate logical material/progress read capabilities await actual generation
queries; a third snapshot repository was not introduced.

The current source/storage contracts intentionally remain scoped to normalized
WaniKani data and one account per store. They do not yet provide a generic
multi-provider ontology, multi-tenant authorization, or asynchronous SQL/remote
storage. `ARCHITECTURE.md` records these limits, all six scenario checks, future
generation/Cloud composition, and public code/private asset repository boundaries.
No new dependency, crate, runtime feature, cache schema, or future placeholder was
added. Source API changes include the new entry points and an owned `Summary`
(no lifetime parameter); only the username string is copied, not source vectors.

### TDD and refactor record

Each new behavioral slice began with the listed failing test. Module relocation,
documentation, and routing existing callers through a tested use case were
refactors rather than artificial behavioral RED cycles.

| Cycle | Observed RED | GREEN and explicit refactor review |
| --- | --- | --- |
| Store publication/read contract | Store adapters and ports absent; shared tests failed to compile | Both stores publish complete versions, old readers retain their version, file construction is lazy and schema 1 persists. Reviewed duplication and ownership; used `Arc` reads without cloning the source graph. Focused tests passed. |
| Memory replacement integrity | Memory accepted an invalid synchronization interval | Validate before publication and reject a different account; shared memory/file assertions verify preservation and subsequent writes. Reviewed responsibilities and error paths; domain validation remains shared and backend commit checks remain local. Focused tests passed. |
| Writer reservation | A second memory handle could reserve a concurrent writer | Added a private shared state and an owned reservation released by `Drop`; data locks cover only reads/publication. Reviewed ownership, cancellation, and naming; no mutex guard crosses an await. Six store tests passed after formatting/refactor review. |
| App and owned results | `App` and `LearningSource` absent | Same sync/status use case passes with both stores, and results outlive the App. Reviewed result ownership; an owned summary avoids a second summary type or cloning complete input data. App/store/summary tests passed. |
| Real source composition | WaniKani client and borrowed client did not implement `LearningSource` | Added delegation to the existing HTTP adapter, including borrowed clients. Real HTTP publishes to memory; existing full sync/failure/process tests now call App. Reviewed shared flow and removed duplicate orchestration from CLI/test helpers; focused App, sync, CLI and binary tests passed. |

Additional acceptance assertions passed for cancelling a polled sync (including
`Send` futures with both stores), source/validation failure preservation, and
locked/corrupt stores preventing retrieval while retaining typed errors. These
verify guarantees already supplied by the composed validation and writer guards;
they are not reported as additional observed RED cycles.

Final refactor review covered all new and moved production/test code, names,
module direction, typed errors, ownership, duplicated flow, and Rust idioms.
Kept concrete domain calculations, private DTOs, schema-1 envelopes, and the
existing file fault/interruption seams. No additional abstraction was justified.
No production panic shortcuts, new global state, or environment mutation were
introduced. Existing source/file tests and the full suite passed after review.

### Verification

Native macOS/arm64 with the pinned Rust 1.98.1 passed:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all`: 69 test entries passed (25 library, 1 binary,
  5 App, 13 cache, 8 CLI, 6 store, 6 summary, 5 sync), including two existing
  subprocess helpers; none ignored. One additional rustdoc example compiled.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`
- `git diff --check`; reviewed new/untracked files as well as tracked diffs.

Local HTTP tests ran with authorization to bind loopback sockets. No live API,
credentials, model, Linux/container, or hosted CI run was used for this migration.
Earlier platform results are historical only. Real file fault/process tests
remain coverage of their stated boundaries, not a power-loss simulation. Request
deadlines still do not bound the total refresh duration or collection size.

Rust notes for a Ruby developer: generic `App<Store, Source>` checks adapter
contracts at compile time without a dependency container. An owned writer token
releases its reservation on return, failure, or cancellation. `Arc` shares an
immutable version, while an owned summary can outlive both App and source data.

Next steps remain the separately authorized milestones below: learner/progress
and material read models with knowledge derivation, then validated generation
using explicit prompt/model dependencies. This migration does not start them.

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

For future generation tests, run real knowledge derivation, prompt preparation,
response parsing, and validation with scripted model responses at the model
boundary. Call counts are appropriate when they verify a cost or retry budget;
internal helper-call sequences are not contracts. Scripted responses never serve
as automatic production fallbacks or evidence of real model quality.

Use real in-memory adapters for application tests when their storage semantics
are sufficient. Exercise common invariants across implementations and verify
backend-specific durability and concurrency against the actual backend. Retain
local HTTP and real-file tests; future PostgreSQL tests use an isolated database.
An in-memory store does not test SQL or filesystem behavior. Do not require
`create_null()`, a test mode, or a new trait for every component.

Selected influences: [Testing Without Mocks](https://www.jamesshore.com/v2/projects/nullables/testing-without-mocks)
for behavioral tests with real collaborators and explicit infrastructure
boundaries, and [CQRS](https://martinfowler.com/bliki/CQRS.html) for distinguishing
update models from derived read models. Neither is a mandatory architecture kit.

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

These follow G0 and require separate implementation authorization. They are not
part of the completed sync/status milestone or the first Cloud preview task:

1. **Learner constraints and retrieval:** grammar knowledge as learner data,
   initially entered through a local file, an explicit revisable
   `LearnerKnowledgePolicy`, manual targets, and structured lexical retrieval.
   Targets identify the word, intended reading, and intended sense. Acceptance: explainable
   target/context selection from real learner data with no mandatory vector
   search. When database persistence is introduced, grammar belongs alongside
   other learner data; files may remain import/export. Derive `LearnerKnowledge`
   on demand from preserved progress and manual declarations. Future provider
   grammar identifiers remain independent; no semantic cross-provider mapping
   or canonical catalog is required.
2. **Validated generation:** evidence-backed Japanese analysis and explicit
   pass/fail/inconclusive checks, real best-of-two generation, a minimal combined
   naturalness/coherence review, and bounded repair for sentences and stories.
   Include focused, grounded sense/reading checks when supporting ambiguous
   targets. Acceptance: select an acceptable passage or report failure
   without silently relaxing constraints; length and complexity do not authorize
   unfamiliar vocabulary or kanji. Use the `ExerciseGenerator` responsibility,
   with candidate production behind `CandidateGenerator`, prompt sets from
   `PromptStore`, and an explicitly selected `LanguageModel`. Introduce only the
   substitution points demonstrated by the implementing milestone. Bound model
   invocations per phase and globally, including retries; repaired candidates
   receive fresh analysis and full reassessment.
3. **Reading practice:** reading quizzes with kana/romaji normalization and
   persisted attempts. Acceptance: deterministic comparison against contextually
   validated readings and useful mistake records. Rephrase or reject ambiguous
   prompts; do not penalize another contextually valid reading solely because it
   differs from the intended target.
4. **Meaning and usage evaluation:** multilingual translation judging and grounded
   semantic criticism beyond the focused target checks already introduced.
   Acceptance: evaluate preserved meaning without requiring one canonical
   translation; extend the basic naturalness/coherence review and focused
   reading/sense assessment already introduced with generation.
5. **Adaptive selection:** combine WaniKani statistics with Yomibu mistakes.
   Acceptance: target choices use both sources and can be explained without
   introducing an SRS.

Bunpro, JMdict, external corpora, web/native/MCP interfaces, PostgreSQL, and vector
retrieval remain conditional future work. Revisit them only when a concrete
product or retrieval need justifies their cost.
