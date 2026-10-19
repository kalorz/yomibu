# WaniKani Storage Groups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task after separate implementation authorization. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement finite atomic WaniKani save groups and explicit successful-sync completion through storage ports, verified with synthetic inputs and real PostgreSQL.

**Architecture:** Core domain supplies identifiers/material kinds; core infrastructure contracts own prepared storage requests and their input limits. The PostgreSQL adapter owns persisted JSON encoding, backend representation checks, grouped SQL and transaction outcomes. The future application owns reading, grouping, traversal verification and combining group outcomes. Extend the existing adapter without adding an application operation or CLI command.

**Tech Stack:** Existing Rust toolchain, PostgreSQL 18+/pgvector, SQLx 0.9 and Tokio; `chrono` for explicit UTC timestamps and `serde_json` for opaque JSON values.

**Spec:** [#109](https://github.com/kalorz/yomibu/issues/109), its authoritative parent [#97](https://github.com/kalorz/yomibu/issues/97), [AGENTS.md](../../../AGENTS.md), [ARCHITECTURE.md](../../../ARCHITECTURE.md), [GLOSSARY.md](../../../GLOSSARY.md), and the [retained SQL schema](../../plan/schema.postgresql.sql). These remain authoritative; this file records implementation choices and execution steps, not another design specification.

**Baseline:** Fresh plan against `main` at `cf0b2b29aa3591acc5d8dc17416d23a915e4908c`, including merged PR #110, inspected on 2026-10-19. Historical #106 is not needed for this plan. Status: plan approved; implementation separately authorized and in progress.

## Global constraints

- Preserve #109's guarantees and exclusions, especially its “Group outcomes and whole-sync completion” and “Concurrency and resource limits” sections; retain only `last_import_completed_at`.
- Follow the existing [dependency boundaries](../../../ARCHITECTURE.md#boundaries), [ingestion boundaries](../../../ARCHITECTURE.md#ingestion-boundaries), [schema policy](../../../ARCHITECTURE.md#schema-changes) and [terminology](../../../GLOSSARY.md#ingestion-wording).
- No HTTP/parsing, credentials, application orchestration, CLI wiring, standalone shared-subject synchronization, migration/attempt/resume framework, staging workflow or entry resolution. No automatic write retry, including deadlock rollback.
- Requirements come from the links above. Names, module layout, JSON envelope, SQL shape, chunk sizes and library features below are revisable implementation choices; changes must preserve the approved behavior.

## Existing verification and gaps

The existing infrastructure module has collection-ID lookup and learner creation ports. `PostgresLearnerStorage` uses an injected `PgPool`, absolute deadlines and sanitized errors. Its autocommit learner-write error classification is **not** an explicit-transaction outcome classifier.

The retained schema already supplies shared collection uniqueness, subject identity, account-scoped source uniqueness, progress primary keys and both same-collection foreign keys. `recorded_at` is the observation timestamp; account/progress source update times belong in JSON. No schema change is currently necessary.

`crates/yomibu-ext/tests/postgres.rs` currently has seven ignored real-PostgreSQL tests covering collection lookup, learner creation, source identity/ownership/references, deadlines and abandoned-connection recovery. CI also runs a CLI PostgreSQL wiring test. Those tests do not establish import atomicity, freshness, completion or lost-commit-response behavior. All checks below are proposed, not results already obtained.

Use the commands and prerequisites in [CI](../../../.github/workflows/ci.yml) and [README development notes](../../../README.md#developing), rather than maintaining a second command list here. The PostgreSQL job's ignored-test command must discover the new adapter tests; the quality jobs already cover Rust checks and architecture enforcement.

## Review focus

1. A conflict winner invisible to an earlier snapshot must still resolve on the following statement; a no-op upsert must not look like a missing identity (Task 2).
2. An older payload with a newer observation must update only `recorded_at`, while omission must update neither (Task 2).
3. Deferred commit rejection, lost commit acknowledgement and late confirmed commit must remain distinct, with bounded connection recovery (Task 3).
4. Storage-contract input limits and adapter encoded-argument limits must each be enforced at their owning boundary; a single oversized record must fail before database work (Tasks 2 and 5).
5. Concurrent completion must retain the maximum timestamp without changing account payloads or ordinary-save behavior (Task 4).

## Responsibilities and affected files

Paths below are proposed implementation changes, not files changed by this planning request.

| Files | Responsibility |
| --- | --- |
| Modify `crates/yomibu-core/src/domain/mod.rs` | Add `LearnerProgressSourceId` and the material-kind value used by storage requests; no JSON encoding, resource limits or PostgreSQL validation in domain. |
| Create `crates/yomibu-core/src/ports/infrastructure/wanikani.rs`; modify its parent `mod.rs` | Narrow lookup/save/completion contracts, prepared request values, input validation/limits, explicit outcomes and deadline/drop semantics. |
| Create `crates/yomibu-ext/src/postgres/wanikani.rs` | Implement the ports on `PostgresLearnerStorage`: fixed shared collection identity, bounded SQL, persisted JSON envelopes, encoded-size/backend representation checks, references and freshness. |
| Create `crates/yomibu-ext/src/postgres/transaction.rs`; modify `postgres.rs` | Private explicit-transaction lifecycle used by group saves and completion; reuse the existing sanitized error helper. Keep existing learner creation semantics unchanged. |
| Create `crates/yomibu-ext/tests/postgres_wanikani.rs`, `tests/support/mod.rs`, `tests/support/wanikani.rs`, `tests/support/postgres_faults.rs`; modify `tests/postgres.rs` | Extract only reusable isolated-schema/pool setup; add synthetic fixtures, independently runnable contract scenarios and narrowly scoped PostgreSQL fault controls. |
| Create `crates/yomibu-ext/tests/postgres_wanikani_resources.rs` | Reproducible storage sizing scenarios, measurements and structural resource assertions. |
| Modify root/core/ext `Cargo.toml`, `Cargo.lock`, `bylaw.toml`; core `clippy.toml` if resolved features expose clock APIs | Add timestamp/JSON features and review the pure `serde_json` allowlist addition; test-only Tokio network/I/O features for fault injection. |
| Modify `README.md` | Link to the port guarantees and describe the new PostgreSQL checks and resource reproduction, without duplicating the ticket. |

No application or CLI file needs an import change. No glossary change is anticipated: all domain concepts already have agreed meanings. Preserve `docs/plan/` unchanged.

## Proposed contracts and validation ownership

Place the following request/receipt/error types in `ports::infrastructure::wanikani`; they are storage-contract representations of supplied Yomibu data, not domain entities or HTTP responses. Only `LearnerProgressSourceId` and `MaterialKind` belong in `domain/mod.rs`.

- `SourceRecord { updated_at: DateTime<Utc>, payload: Box<serde_json::value::RawValue> }`: complete opaque JSON object with an explicit record-specific update time. `SourceRecord::new(updated_at: DateTime<Utc>, raw_json: &str) -> Result<Self, InvalidSaveGroup>` checks the contract's raw byte limit before copying/parsing and validates JSON/object shape. It imposes no PostgreSQL precision/range or persisted-envelope rules. Do not inspect provider fields or infer freshness from local time.
- `PreparedSubject { external_id: u64, kind: Option<MaterialKind>, source: SourceRecord }`. Define the domain value `MaterialKind::{Vocabulary, Grammar}`; this WaniKani storage contract rejects `Grammar`. Vocabulary/kana vocabulary map to vocabulary; kanji/radicals map to `None` at the supplying boundary, with their type retained in the payload. No grammar import behavior is introduced here.
- `PreparedProgress { subject_external_id: u64, progress_stage: Option<String>, is_hidden: bool, learning_started_at: Option<DateTime<Utc>>, first_passed_at: Option<DateTime<Utc>>, observed_at: DateTime<Utc>, source: SourceRecord }`. No `is_marked_known` input. Exclusion data remains in the progress payload/translated `is_hidden` value.
- `PreparedSaveGroup { learner_id: LearnerId, external_account_id: String, account: SourceRecord, subjects: Vec<PreparedSubject>, progress: Vec<PreparedProgress> }`, with private fields and a fallible constructor. Account ID is the supplied `/user.data.id`, retained exactly, never a token. Reject empty IDs, zero subject IDs, duplicate subject IDs and duplicate progress references rather than picking a winner. Permit no subjects/progress, and progress referencing subjects absent from this input.
- `LearnerProgressSourceId(uuid::Uuid)`, `SaveReceipt { collection_id: CollectionId, learner_progress_source_id: LearnerProgressSourceId }`, and `CompletionReceipt { learner_progress_source_id: LearnerProgressSourceId, last_import_completed_at: DateTime<Utc> }`. Receipts contain confirmed facts, not attempted/inserted counts or traversal claims.
- `ImportWriteError::{NotSaved(String), CompletionUnknown(String)}`: sanitized, scoped to **this atomic write**. `NotSaved` may mean rejected before dispatch or proven not committed; do not describe it as an acknowledged rollback unless one was observed. No whole-sync result type is needed in this ticket.

Proposed storage-contract constructor: `PreparedSaveGroup::new(learner_id, external_account_id, account, subjects, progress) -> Result<Self, InvalidSaveGroup>`, using the field types above. Keep `SourceRecord` and `PreparedSaveGroup` fields private with read-only accessors. `PreparedSubject` and `PreparedProgress` are plain input structs; group construction validates/owns them and exposes only read-only slices afterward, so callers cannot bypass the group's preconditions. Supply explicit timestamps; never read an ambient clock in domain.

Storage-contract defaults: at most 1,000 subjects and 1,000 progress records, at most 8 MiB of variable-size input data including account data, at most 1 MiB of raw JSON per source payload. Define input bytes as the UTF-8 lengths of account ID, progress-stage strings and all raw payloads, summed with checked arithmetic; the record counts bound fixed-size fields. This measure contains no persisted JSON envelope, SQLx argument or PostgreSQL protocol knowledge. Group construction takes ownership of existing vectors without cloning payloads and checks counts/total bytes before producing the validated request. The future caller remains responsible for bounded construction of its inputs. These are revisable **per-call** choices, not inventory caps or API page sizes. The lookup contract accepts at most 1,000 IDs; empty lookup returns empty without I/O.

| Owning boundary | Validation and failure |
| --- | --- |
| Core domain | Typed persistence IDs/material kinds only in this slice; no serialization, resource budgeting or backend representation rules. |
| Core storage contract | Request identity/uniqueness/kind preconditions, opaque JSON/object validity, record counts and raw input-byte limits. Constructors return `InvalidSaveGroup`; lookup argument rejection returns `StorageError`. Neither performs I/O. |
| PostgreSQL adapter, before acquisition | Exact timestamp representability for every source/observation/learning/completion time, persisted JSON encoding and bounded encoded-argument sizing. Reject loss of sub-microsecond precision or an unsupported finite range rather than rounding freshness values. Adapter preflight failures return sanitized `ImportWriteError::NotSaved`; no database work occurs. |
| PostgreSQL adapter, inside transaction | Database-dependent existence, ownership, freshness metadata, uniqueness/FKs and JSONB representability. Preserve native PostgreSQL rejection/rollback for JSON that its JSONB type cannot represent; do not build a custom JSONB validator or put backend rules in the contract constructor. |
| Future caller | Source translation/credential exclusion, bounded upstream input preparation, operation-wide budgets and complete traversal; unchanged and outside this task. |

The adapter accounts for its own envelope, driver/protocol overhead and chunk buffers separately from the raw-input budget. Perform a bounded encoding/sizing preflight before acquisition to reject oversized individual writes, then encode/release one SQL chunk at a time. Do not retain a second encoding of the whole group. A valid contract request can still receive a documented backend representation rejection; validation must not silently truncate payloads or timestamps.

Use the existing boxed-future style, with explicit lifetimes tying borrowed values to the call. This shows the **final** API; introduce each declaration with its complete implementation in the task marked below:

```rust
trait WaniKaniSubjectQuery { // Task 1
    fn existing_subject_ids<'a>(&'a self, ids: &'a [u64], deadline: Instant)
        -> Pin<Box<dyn Future<Output = Result<Vec<u64>, StorageError>> + 'a>>;
}
trait WaniKaniGroupWriter { // Introduce with save_group only in Task 2
    fn save_group<'a>(&'a self, group: &'a PreparedSaveGroup, deadline: Instant)
        -> Pin<Box<dyn Future<Output = Result<SaveReceipt, ImportWriteError>> + 'a>>;
    // Add this required method and its implementation together in Task 4.
    fn record_completion(&self, source_id: LearnerProgressSourceId,
        completed_at: DateTime<Utc>, deadline: Instant)
        -> Pin<Box<dyn Future<Output = Result<CompletionReceipt, ImportWriteError>> + '_>>;
}
```

Lookup returns unique requested external IDs in deterministic ascending order from the shared WaniKani collection; absent collection means no matches, never creation. Reject over-limit/zero IDs; repeated lookup IDs may be deduplicated. A successful lookup is advisory: save performs final database resolution. Completion requires an existing WaniKani progress source; it neither creates one nor verifies traversal. Its caller must first establish all required reads and saves succeeded, including an account-only save for an empty traversal.

Minimal dependency construction in adapter tests: `let storage = PostgresLearnerStorage::new(pool.clone());` call the query/writer traits on that value with one caller-supplied deadline. A supplied `group` can contain progress for IDs returned by lookup and only newly obtained subjects. Retain each receipt; call `record_completion(receipt.learner_progress_source_id, completed_at, deadline)` only when the test's declared input is exhausted and all saves are confirmed. This is test composition, not a new production orchestration layer.

Adapter-private JSON storage choice: subjects retain the complete payload directly in `subjects.source_data` and use `source_updated_at`; account and progress use `{"source_updated_at": <canonical UTC timestamp>, "payload": <complete original JSON object>}` in their `source_data`. The adapter alone constructs/parses this envelope and formats its timestamps. This preserves every original field without colliding with provider names. Equal/older data preserves the entire saved source envelope and translated fields. Stored missing/malformed update metadata encountered during replacement is an explicit safe failure, not permission to overwrite using local time; direct SQL fixtures for the new contract must carry valid metadata. JSONB preserves JSON content, not source whitespace/key order. Opaque payloads are credential-free by precondition; there is no credential-bearing input type or credential scanner.

Use `chrono` with defaults disabled and only `std`, and `serde_json` with `raw_value` in the storage-contract/adapter modules; enable SQLx's corresponding timestamp/JSON support. `chrono` is already reviewed in Bylaw; review `serde_json` for opaque storage payloads and permit only that dependency. Do not move encoding into domain merely because the library is pure. Check resolved features and extend scoped Clippy restrictions if any dependency enables ambient Chrono clock APIs. Avoid a bespoke timestamp/JSON parser and a generic import framework.

## Task dependencies and commit gate

Execute Tasks 1 → 2 → 3 → 4 → 5. The final API above is not a scaffold to declare wholesale in Task 1.

| Task | Complete interface available at its commit | New dependencies introduced with their first consumer |
| --- | --- | --- |
| 1 | `WaniKaniSubjectQuery`, fully implemented on the existing adapter | Existing stack only; lookup validation and shared PostgreSQL test setup |
| 2 | Query plus `WaniKaniGroupWriter` with **only** `save_group` | Prepared storage values/constructors, domain ID/kind, save receipt/errors, Chrono/JSON features, adapter encoding and complete transaction lifecycle |
| 3 | Same public API as Task 2 | Fault relay/test features and deeper lifecycle verification; no dependency on completion |
| 4 | Add `record_completion` to the writer and every implementation together; add `CompletionReceipt` | Reuse Task 2 values/encoding checks and Task 3 fault controls |
| 5 | Same complete public API as Task 4 | Resource scenarios use the existing ports, fixtures and relay counters |

Every task ends by running CI's all-target Clippy, workspace Rust tests and Rustdoc commands, plus its affected real-PostgreSQL checks; run architecture checks when that task changes contracts/dependencies. All targets must compile and current tests pass before committing. Add module declarations, exports, tests and fixture helpers only with the files/implementations they use. No `todo!`, `unimplemented!`, panic/default-error trait stubs, unused future-task scaffolding or lint suppression to bridge tasks. A temporary failing test is part of development, never the committed increment. These are future implementation gates, not compilation results from this planning revision.

## Task 1: Grouped subject lookup

**Files:** Create the core port and adapter `wanikani.rs` modules and their parent declarations; create shared `tests/support/mod.rs` and `postgres_wanikani.rs`; extract the existing schema/pool setup from `tests/postgres.rs`. No new timestamp/JSON dependencies or domain values yet.

**Interfaces:** Produce only `WaniKaniSubjectQuery`, its lookup argument rules and its complete adapter implementation, reusing existing `StorageError`. No writer trait, save request/receipt types or completion method is declared in this increment.

- [x] Add independent PostgreSQL tests `lookup_is_grouped_read_only_and_collection_scoped`, `lookup_rejects_invalid_or_over_limit_input`, and `lookup_obeys_deadline`. Seed another collection with the same external ID and verify only real WaniKani matches are returned once; absent collection and empty IDs create nothing. Assert zero IDs and over-limit input fail before acquisition, at-limit input works and repeated IDs are deduplicated. Exercise expired/acquisition/lock deadlines separately from valid absence.
- [x] Run the new PostgreSQL test target using existing CI invocation patterns; confirm failure is the missing implementation, not a missing database prerequisite.
- [x] Implement lookup and enforce its contract argument limits before I/O. Use one `ANY`-array query joining the shared collection, with bounded output and the unchanged absolute deadline. Choose fixed `(provider, key, name) = ('wanikani', 'subjects', 'WaniKani')`, `owner_learner_id = NULL`, `is_reference = FALSE`; this key is internal and stable across calls. Do not create collections in the read path.
- [x] Run the common commit gate and affected architecture checks. Expected: the implemented lookup and all existing targets compile/pass without declarations or references to later-task APIs.
- [x] Commit the complete lookup increment after implementation is authorized.

## Task 2: Atomic group persistence, freshness and observation

**Files:** Core `domain/mod.rs` and port `wanikani.rs`; root/core/ext manifests, lockfile and reviewed-library configuration; adapter `postgres/wanikani.rs`, new `postgres/transaction.rs`, `postgres.rs`; `postgres_wanikani.rs` and new `tests/support/wanikani.rs`. Update scoped Clippy configuration only if resolved clock features require it.

**Interfaces:** Retain Task 1's complete lookup. Introduce the prepared storage request types/constructors, `InvalidSaveGroup`, domain ID/kind, `SaveReceipt` and `ImportWriteError` with their consumers. Declare `WaniKaniGroupWriter` with **only `save_group`** and implement that entire trait in this increment. The private transaction helper owns a pool lease and produces a receipt only after confirmed commit; implement the lifecycle/outcome rules detailed in Task 3 now. Task 3 expands fault verification without being a compile dependency or a deferred production implementation. `CompletionReceipt` and `record_completion` do not exist yet.

- [x] Add storage-contract unit tests `group_accepts_account_only_and_existing_subject_references`, `group_rejects_duplicate_identities`, `source_record_validates_opaque_json_and_raw_bytes`, and `group_limits_count_utf8_bytes_and_reject_oversized_input`. Assert valid empty progress, references absent from supplied subjects, invalid/duplicate IDs, rejected grammar kind, object-shaped JSON, multibyte counts and exact/one-byte-over limits. These test raw input sizes, not SQL envelopes or timestamp precision. A nanosecond timestamp remains a valid contract value; backend acceptance is separate.
- [x] Add adapter tests `encoding_preserves_payload_and_accounts_for_envelope_bytes` and `postgres_representation_checks_belong_to_the_adapter`. Assert all timestamp fields are checked for lossless PostgreSQL encoding; a sub-microsecond input rejected by adapter preflight returns sanitized `NotSaved` without acquisition. Exercise encoded-size boundaries separately from raw-input limits. Against real PostgreSQL, use valid JSON containing an escaped NUL that JSONB rejects; assert sanitized `NotSaved`, no silent payload modification and rollback of preceding writes. Keep stored-freshness metadata checks below at the adapter as well.

- [x] Add independently runnable scenarios: `first_save_and_replay_all_subject_types`, `subjects_are_shared_progress_and_access_are_independent`, `reuse_subjects_and_resolve_a_concurrent_insert`, `unknown_learner_and_missing_subject_save_nothing`, `mixed_freshness_updates_each_record_independently`, `invalid_stored_freshness_rolls_back_the_group`, `observations_advance_without_source_replacement`, `account_only_save_preserves_existing_progress`, and `failure_in_later_chunk_rolls_back_only_its_group`.
- [x] Pin assertions to #109's corresponding acceptance bullets: stable collection/subject/source IDs and one progress row per source/subject; full payload retention and NULL `is_marked_known`; zero entries/details/new learners; account A/B and learner isolation; new/equal/old account, subject and progress combinations; newer demotions/resets and explicit NULL dates; later observations on stale data, identical replay and omission; earlier committed group survives. Compare complete relevant row snapshots, not only counts. Missing-subject tests must use a valid learner/source/collection so another constraint cannot mask the reference rule. Include the same external subject ID in a different collection. For the selected invalid-stored-metadata behavior, separately seed a NULL subject timestamp and missing/malformed account/progress JSON timestamps; require sanitized `NotSaved` and rollback of preceding writes, not a silent no-op.
- [x] Run the focused storage-contract/adapter unit tests and real-PostgreSQL cases; observe failures for absent constructor/save behavior.
- [x] Implement the storage-contract constructors and adapter preflight at the boundaries in the ownership table. Add their dependency features with these consumers, inspect resolved owners, and keep persisted-envelope construction and PostgreSQL timestamp checks entirely in `yomibu-ext`. The absolute deadline includes adapter preparation: check it before preparation and again before acquisition; preparation is finite cooperative CPU work, not a new time budget.
- [x] Implement one explicit transaction, with this order and no source I/O:
  1. Insert shared collection `ON CONFLICT ... WHERE owner_learner_id IS NULL DO NOTHING`, then select its ID in a **separate READ COMMITTED statement**. Do not update/lock that shared row on every save or use a same-statement CTE fallback that misses a concurrent conflict winner.
  2. Upsert the learner/collection/account source with atomic strictly-newer timestamp replacement, comparing JSON timestamps as `timestamptz`, not text; resolve its stable ID separately when an equal/older upsert returns no row. Preserve `last_import_completed_at`. Unknown learner fails through the FK and rolls back all provisional writes.
  3. Sort supplied subjects by external ID; multirow upsert with `WHERE excluded.source_updated_at > subjects.source_updated_at`. Keep IDs stable and propagate newer kind/payload together. Detect unusable existing freshness metadata before replacement; fail safely rather than silently ignoring required input.
  4. Resolve the union of supplied/referenced subject IDs in grouped SELECTs **after** subject writes, scoped to this collection. Reject any missing required ID; do not fabricate parents. Existing records and inserts committed since preliminary lookup are valid. The retained composite FKs are the final same-collection guarantee, including concurrent deletion.
  5. Sort progress by resolved subject UUID. Multirow upsert on `(learner_progress_source_id, subject_id)` compares JSON timestamps as `timestamptz` for each source-derived field, and independently uses `GREATEST(saved.recorded_at, incoming.recorded_at)`. Use conditional assignments, not a freshness `WHERE` that would suppress observation-only updates. Skip a physical update only when neither data nor observation advances. Keep `is_marked_known` NULL. Do not coalesce away newer explicit NULL dates, delete omitted rows or touch unrelated source metadata.
  6. Commit once, after every internal SQL chunk succeeds; produce the bounded receipt only after commit confirmation.
- [x] In the adapter, bound SQL chunks initially to 250 rows and 1 MiB encoded arguments, and always below PostgreSQL's 65,535-parameter limit. Bind data, including JSON, rather than interpolating it; query text varies only with bounded row counts. Account for the adapter's envelope/protocol overhead separately from storage-contract input bytes; allow a single valid record to use a dedicated statement up to the 8 MiB hard encoded-argument bound if it exceeds the ordinary chunk target. Preflight rejects an oversized encoded record before I/O, using bounded scratch/counting rather than retaining whole-group encodings. Encode/release one actual SQL chunk at a time. Return identity projections, not payloads.
- [x] Add controlled concurrent first-save and reimport cases: two accounts/learners against a shared collection, the same source with mixed source/observation order, and reverse-ordered input IDs. Use independent connections and database/barrier evidence, not sleep-based race guesses. Verify no duplicates, stale overwrite or observation regression. Use native uniqueness/FKs and consistent source → subjects → progress lock ordering; no advisory/collection-wide lock and no retry loop.
- [x] Run the common commit gate, affected architecture/enforcement checks and existing/new PostgreSQL contracts. Expected: all current trait implementations compile with no completion stub; contract/adapter ownership is preserved, saved rows satisfy acceptance assertions, and a trigger failure in a later SQL chunk leaves none of that group durable. Commit this complete persistence increment.

## Task 3: Deadline, rollback and commit-outcome verification

**Files:** `postgres/transaction.rs`, `postgres/wanikani.rs`, `postgres_wanikani.rs`, `tests/support/postgres_faults.rs`; ext test features as needed.

**Interfaces:** Keep Task 2's complete save-only writer, `Result<SaveReceipt, ImportWriteError>` and caller's absolute `Instant`. No new public method is required. Reuse this lifecycle for Task 4 completion; no runtime/SQLx types in core signatures.

The following lifecycle is implemented in Task 2 and further verified here. Use SQLx explicit `Transaction` over a guarded pool lease. Reject expiry before acquisition, after acquisition and before each new statement/commit dispatch. Wrap acquisition, BEGIN, all statements and COMMIT in the same absolute deadline; never reset it for a chunk. Transaction-local statement/lock limits derive from remaining time, rounded up to at least 1 ms (zero disables PostgreSQL timeouts). Keep finite session limits as the composition prerequisite. No blanket lock or worker task per write.

Track whether COMMIT might have been dispatched, separately from receipt construction and connection cleanup:

| Evidence | Result and cleanup |
| --- | --- |
| Invalid/expired input, acquisition failure, or no COMMIT ever dispatched | `NotSaved`. No durable transaction write is possible; discard an interrupted connection so buffered work cannot later be committed/reused. |
| Statement rejection with acknowledged rollback, or positively identified transaction-aborting server error such as `40P01` | `NotSaved`; no retry. Roll back explicitly while a bounded cleanup budget remains. |
| COMMIT dispatched and an explicit deferred-constraint/transaction-abort rejection proves no commit | `NotSaved`; classify by verified SQLSTATE/severity/protocol phase, not provider text. |
| COMMIT may have been dispatched, then timeout, connection/protocol loss or `40003` | `CompletionUnknown`, even if later cleanup/ROLLBACK succeeds. No rollback inference from a timeout or dropped future. |
| Successful COMMIT response fully observed | Return receipt, even if the deadline is now exceeded or cleanup later fails. Never run the read port's late-success rejection afterward. |

Make rollback cleanup at most 250 ms outside an exhausted operation deadline, then discard the lease; do not await an abandoned statement's drain indefinitely. Initially reuse the existing close-on-drop guard pattern, adapting it to distinguish commit/rollback confirmation from reusable connection state. SQLx 0.9's close-on-drop itself has a finite 5-second cleanup bound; recovery tests must cover that bound plus connection establishment, not assume immediate recovery. Do not return an interrupted connection normally to the sole-slot pool. A dropped future abandons delivery of its outcome; the future application must conservatively retain an unknown outcome for a dispatched write whose receipt it abandoned. This ticket does not introduce a cancellation-token API or durable outcome recovery.

- [ ] Add outcome-classifier tests covering every row above, especially deferred rejection vs fatal/unknown errors and `40P01` without retry. Assert errors contain no fixture payload, account data, URL, SQL text or injected diagnostic secret.
- [ ] Add independent real-PostgreSQL tests for expired/acquisition/lock/statement deadlines, rollback after prior writes, cancellation before commit, backend termination before commit, deferred commit rejection, known deadlock rollback and bounded sole-slot recovery. Assert both returned knowledge (where a result exists) and durable state through an independent observer connection. For cancellation, explicitly drop the future and assert recovery/durable rollback; do not pretend it returned an error.
- [ ] Add a small **test-only**, frame-aware local PostgreSQL relay that can gate the COMMIT request/response on a dedicated synthetic connection. Consume a successful backend COMMIT response, withhold it and disconnect: caller must report unknown while the observer sees the saved group. Also drop the request before it reaches PostgreSQL: caller still reports unknown after dispatch, observer sees no save. This proves uncertainty's two possible durable outcomes without probabilistic network races. Restrict the helper to trusted disposable local tests; no production proxy abstraction.
- [ ] Verify confirmed completion observed late with a real successful commit and controlled polling/response delivery; once the commit future yields success, advancing deadline observation cannot change the receipt. Separately unit-test the final result handling using explicit instants. No wall-clock sleep is the sole synchronizer.
- [ ] Run failure tests against Task 2's complete implementation, then correct any exposed defect in the private lifecycle/error mapping. If it already passes a new case, keep the meaningful test; do not introduce a failure just to manufacture a red step. Inspect SQLx/Tokio behavior at the locked versions when choosing the polling/cleanup mechanism; never equate a `RETURNING` row or transaction drop with a confirmed commit/rollback.
- [ ] Run the common commit gate and all new/existing PostgreSQL tests with watchdogs exceeding the documented cleanup/acquisition bounds. Expected: the unchanged public API compiles, each outcome/durable-state assertion passes, no retry is hidden and abandoned work cannot indefinitely occupy the only pool slot. Commit the verified outcome increment.

## Task 4: Explicit successful-sync completion

**Files:** Port `wanikani.rs`, adapter `postgres/wanikani.rs`, `postgres_wanikani.rs`.

**Interfaces:** Add `CompletionReceipt` and the required `WaniKaniGroupWriter::record_completion` declaration **together with its full PostgreSQL implementation**, using Task 3's verified lifecycle and Task 2's adapter timestamp checks. Update every supplied test implementation in this same increment; do not add a default trait body. Return a receipt only after confirmed commit.

- [ ] Add tests `ordinary_saves_do_not_record_completion`, `completion_after_account_only_save`, `overlapping_completions_do_not_regress`, `completion_failure_preserves_prior_groups`, and `completion_rejects_unknown_or_non_wanikani_source`. Assert the retained schema has no `last_import_started_at` column. Save ordinary groups both before and after a completion and compare the timestamp unchanged.
- [ ] Run the completion cases to show the missing method/behavior fails.
- [ ] Validate `completed_at` with the adapter's lossless timestamp checks before acquisition; test representation rejection with no database work. Implement a short transaction with an UPDATE scoped through the shared WaniKani collection: set `last_import_completed_at = GREATEST(last_import_completed_at, supplied_completed_at)`, returning the actually stored timestamp/source ID. PostgreSQL's null handling permits the first timestamp. A nonexistent/wrong-provider source is `NotSaved`; there is no upsert or account update here. Test older/equal/newer completions and concurrent order reversal.
- [ ] Exercise the shared lifecycle through this method too: failure before dispatch/rollback leaves the old completion value; lost commit response reports unknown with observer inspection; confirmed late completion retains its receipt. Prior group rows/receipts remain intact in every case. Do not infer eligibility to complete from source existence, group size or row counts.
- [ ] Run the common commit gate, affected architecture checks and full PostgreSQL contracts. Expected: every writer implementation compiles with both methods; only explicit confirmed completion advances metadata; account-only completion is valid, no source data changes, and failure is distinguishable from complete success. Commit this increment.

## Task 5: Storage sizing, integration checks and handoff

**Files:** `postgres_wanikani_resources.rs`, fixture support, `README.md`; touch production chunking only if measurements reveal a concrete bound/performance defect.

**Interfaces:** Use the same public query/write/completion ports; the resource harness is a finite-group fixture caller, not an application importer.

- [ ] Add a deterministic generator yielding one group at a time, approximately 10,000 shared subjects and 10,000 progress records per account in total. Vary all four subject types, multibyte text, nested fields, arrays and long meanings/readings using synthetic full-shape payloads. Report measured payload-size distribution and its synthetic assumptions; no live WaniKani calls or credentials.
- [ ] Run separately reportable cold-storage, populated-subject reuse (zero resupplied subjects), identical/stale replay with later observations, overlapping imports for the same source, and overlapping accounts sharing subjects. Use two active imports initially and pool capacities 1 and 2; a one-connection run verifies queued acquisition while two connections exercise database contention. Add a near-byte-limit case and totals of 1,000 versus 10,000 with group limits held constant.
- [ ] Measure input/encoded payload bytes, maximum retained group/chunk sizes, logical SQL commands and protocol exchanges, elapsed time and process peak memory. Use the test relay's counters for protocol measurements (including setup/parse exchanges separately) and process-isolated runs with the host's resource measurement tool for peak RSS. Report platform, build mode, pool size and group size; do not label a whole parallel test process's RSS as one scenario's memory.
- [ ] Assert structural bounds independently at each owner: the storage-contract constructor rejects over-limit raw groups; the adapter rejects oversized encodings before acquisition. Verify no per-record SQL loop, bounded preflight/argument/returned-ID buffers, expected command growth per chunk rather than per row, no whole-input retention, and atomic rollback across multiple chunks. Compare memory as total input grows with fixed group size; explain allocator/runtime baseline rather than inventing an SLA. Measure overlap's contention/latency without treating approximately 10,000 as a hard cap.
- [ ] Keep a bounded representative smoke scenario in the PostgreSQL CI ignored-test discovery. Gate the slower per-process measurement matrix behind an explicit test-only environment option (document it in README), so normal CI does not silently claim to have collected those measurements. Run the full matrix for this implementation's delivery and report results in the handoff; no new benchmark service, persistent import log or required PostgreSQL extension.
- [ ] Run the common commit gate and remaining affected checks from [CI](../../../.github/workflows/ci.yml): existing PostgreSQL invocation including the new targets, workspace Rust quality checks, and architecture/enforcement checks because shared types/dependencies changed. Verify the retained schema by recreation, not migration. Missing PostgreSQL/Bylaw prerequisites must be reported; a skipped default test is not backend verification.
- [ ] Perform the AGENTS.md simplification and terminology pass across the complete implementation diff and its immediate collaborators. Retain the required ports and fault coverage; remove only redundant machinery. Keep HTTP/workflow resource validation deferred to its owning future task.
- [ ] Commit the resource verification/documentation increment; request implementation review with actual results, new files and additions/deletions. Do not claim all of #97 is implemented.

## Acceptance mapping

| #109 acceptance/verification area | Planned evidence |
| --- | --- |
| Atomic saves, references, missing learner/subject, account-only input | Task 1 lookup; Task 2 storage-contract validation, adapter representation tests, isolated reference tests, reuse/concurrent insert and multi-chunk rollback tests |
| Stable identity, four types, ownership, full source retention, no resolved content | Task 2 first/repeat save and independent account/learner snapshots |
| Independent freshness, demotions/resets/cleared dates, observation/omission | Task 2 mixed/equal/older/newer payload and observation scenarios, including controlled overlap |
| Group outcomes, earlier groups survive, no hidden retry | Tasks 2–3 rollback/deadlock/commit-fault tests with observer state |
| Explicit completion using only `last_import_completed_at` | Task 4 ordinary-save, empty-progress, overlap and separate failure/uncertainty tests |
| Deadlines, cancellation, confirmed/uncertain commit and cleanup | Task 3 lifecycle matrix, real backend faults, late confirmation and sole-slot recovery; reused by Task 4 |
| Finite groups, multirow I/O, realistic sizing and overlap | Task 1 lookup bounds; Task 2 separate raw-input and encoded-argument bounds; Task 5 scenario measurements and CI smoke |
| Owning contracts, architecture and terminology | Per-task compilation/CI gates, ownership-table assertions and final scoped review; no application/CLI import boundary added |

## Planning review and remaining decisions

Focused independent technical review covered the approved issues, repository contracts and locked driver/runtime source. Its recommended tests for NULL/missing/malformed stored freshness metadata, sanitized failure and rollback remain in Task 2. Following user review, the plan was revised to introduce the completion declaration with its implementation in Task 4 and to place request/resource validation in the storage contract and persisted encoding/backend checks in the adapter. A focused independent re-review checked the revised task dependencies, validation ownership and acceptance coverage and found no blockers. Every committed task now has an explicit compilation gate; this is a plan-level review, not a claim that future code has compiled.

Source inspection by both planner and reviewer confirmed that SQLx 0.9 transaction drop queues rollback rather than acknowledging it, pool close-on-drop has a five-second cleanup bound, and Tokio 1.53.3 timeout polls the underlying future before its timer. These are library facts supporting the plan, not proof that an unimplemented adapter passes its contract. No unresolved behavior, scope conflict or material tradeoff blocks planning; there is no remaining decision beyond plan approval and separate implementation authorization.

Planning verification consists of current-main/specification inspection, link/consistency/whitespace checks and independent review. No implementation, PostgreSQL acceptance suite or resource measurement has been performed by this planning request.
