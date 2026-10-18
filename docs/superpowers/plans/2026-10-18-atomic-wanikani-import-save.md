# Atomic WaniKani Import Save Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` for native execution or `superpowers:subagent-driven-development` if the user selects that method. Steps use checkbox syntax for tracking; review the complete branch before delivery.

**Goal:** Save a supplied, validated import batch atomically, preserving identities, per-record freshness and accurate save outcomes.

**Architecture:** A core infrastructure port accepts translated Yomibu values and opaque source payloads. A PostgreSQL adapter owns identity resolution, timestamp comparisons and one transaction. This ticket introduces no application import workflow or CLI command.

**Tech stack:** Existing Rust, SQLx 0.9, Tokio and PostgreSQL 18+ with pgvector; Chrono for UTC values and serde_json for JSON values.

**Spec:** [#109](https://github.com/kalorz/yomibu/issues/109), derived from [#97](https://github.com/kalorz/yomibu/issues/97) and completed design #106. Those descriptions define behavior; the file layout, types and SQL approach below are revisable implementation choices.

## Global constraints

- Follow [AGENTS.md](../../../AGENTS.md), [ARCHITECTURE.md](../../../ARCHITECTURE.md) and [GLOSSARY.md](../../../GLOSSARY.md); the [retained schema](../../plan/schema.postgresql.sql) remains the persistence requirement.
- "No migration mechanism or data-preserving upgrade." No structural schema change is presently needed.
- "Never automatically retry an uncertain write." Keep credentials and raw database errors out of persisted data and presentable errors.
- Exclude HTTP, credentials, provider-response parsing, application orchestration, CLI import wiring, entries, eligibility and generation. Use synthetic fixtures only.

## Review focus

- Concurrent first imports must share identities; concurrent older data must not replace newer data (Task 2).
- Account metadata, subjects and progress can have different freshness within the same batch (Task 2).
- Missing records and newer resets require different behavior: retain missing progress, replace newer explicit values including cleared dates (Task 2).
- A failed late statement or rejected commit must roll back earlier writes and successful-import metadata (Tasks 2–3).
- Lost commit acknowledgment means uncertain completion; a known commit stays confirmed even when observed late (Task 3).

## Files and boundaries

- `crates/yomibu-core/src/ports/infrastructure/import.rs` (new): batch values, structural validation, receipt, write error and storage port; expose through `infrastructure/mod.rs`.
- `crates/yomibu-core/src/domain/mod.rs`: add a progress-source ID value and the material-kind value needed by the contract; preserve existing identities.
- `crates/yomibu-ext/src/postgres_import.rs` (new), exported from `src/lib.rs`: PostgreSQL implementation. Reuse small connection/error helpers from `src/postgres.rs` where appropriate without changing learner operations.
- `crates/yomibu-ext/tests/postgres_import.rs` (new): storage contract tests. Extract only reusable schema/pool setup from `tests/postgres.rs` into `tests/support/mod.rs`; retain its current tests.
- `crates/yomibu-ext/tests/support/commit_proxy.rs` (new): narrowly scoped, test-only lost-acknowledgment control if no existing facility suffices.
- Workspace/core/ext manifests and lockfile: timestamp/JSON dependencies and SQLx codecs; `bylaw.toml`: reviewed JSON value dependency. `README.md`: extend existing test guidance only where needed.

### Task 1: Define the validated batch and storage contract

**Interfaces:** `ImportBatch::new(input: ImportBatchInput) -> Result<ImportBatch, InvalidImportBatch>`; `ImportWriter::save_import<'a>(&'a self, batch: &'a ImportBatch, deadline: Instant) -> Pin<Box<dyn Future<Output = Result<ImportReceipt, ImportWriteError>> + 'a>>`.

Use named input records rather than positional tuples. Keep validated batch fields private with read access for the adapter. Input fields:

| Record | Fields |
| --- | --- |
| Batch | `learner_id: LearnerId`, collection, account, subject/progress vectors, `started_at` and `recorded_at: DateTime<Utc>` |
| Shared collection | `provider`, `key`, `name`: strings; the WaniKani fixture uses `wanikani`, `subjects`, `WaniKani` |
| Account | `external_account_id: String`, `source_updated_at: DateTime<Utc>`, `source_data: serde_json::Value` |
| Subject | `external_id: String`, `kind: Option<MaterialKind>`, source timestamp and payload |
| Progress | `subject_external_id: String`, source timestamp/payload, `progress_stage: Option<String>`, `is_hidden: bool`, nullable `learning_started_at` / `first_passed_at` UTC values |

`MaterialKind` represents the existing vocabulary/grammar schema values. The four provider types remain in payloads; storage does not parse WaniKani responses. The caller must supply credential-free payloads; storage never logs them. No token or `is_marked_known` input exists. `ImportReceipt` contains `collection_id` and `learner_progress_source_id`; `ImportWriteError` distinguishes `NotSaved` and `CompletionUnknown`, with sanitized reasons.

- [ ] Add focused constructor tests: duplicate subject/progress external IDs, empty identity fields and progress lacking a matching batch subject are rejected; a self-contained empty-progress batch and all four mapped subject types are accepted. This validates persistence structure, not provider-response semantics or batch completeness.
- [ ] Run the new tests and confirm failure before implementation. Implement the records, constructor and documented port. Use reviewed Chrono value APIs with clock features disabled. Review serde_json's pure value/serialization use before adding its precise allowance to Bylaw; do not broaden library restrictions generically.
- [ ] Run `cargo test --locked -p yomibu-core`, review visibility and terminology, and commit with a WHY body.

### Task 2: Implement atomic saving and freshness

**Interfaces:** `PostgresImportStorage::new(pool: PgPool) -> Self` implements Task 1's `ImportWriter`. The composition caller continues to configure finite pool/session limits.

- [ ] Add PostgreSQL contract tests for first save/replay, separate accounts/learners, mixed per-record freshness, reset/demotion and missing data. Use Alice/account A with 食べる at stage `5`, newer stage `7`, then a newer stage `1` with cleared dates. Equal/older timestamps with different payloads must not win. Verify four subject types, stable IDs, source metadata isolation, unchanged recording time for untouched progress, and no resolved entry/detail rows.
- [ ] Run `cargo test --locked -p yomibu-ext --test postgres_import -- --ignored` with the disposable PostgreSQL setup documented in README and confirm failure. Implement one explicit transaction: acquire, resolve the shared collection, resolve the progress source, upsert subjects, resolve their IDs, upsert progress, record successful-import metadata, commit. Do not create learners or delete absent records.

Use existing unique keys and atomic SQL timestamp predicates, not read-then-write freshness checks. For the shared collection, use insert-on-conflict then a separate lookup when needed; account for concurrent rows invisible to the insert statement's snapshot. Resolve subject IDs even when an upsert skips stale data. Process conflicting subject/progress keys in deterministic order. Do not serialize every import with a global application lock.

Keep subject freshness in `subjects.source_updated_at`. For account/progress JSON, use a consistent envelope with `source_updated_at` and `payload`, retaining original payload fields inside `payload`. Compare typed timestamps, not strings. SQL NULL (no prior timestamp) accepts the first timestamped record; malformed existing timestamp data must fail instead of silently losing freshness protection. Preserve account payload on equal/older data while advancing successful-import bookkeeping separately. Use the supplied recording time only for inserted/newer progress, and a database completion timestamp near commit for import bookkeeping; neither is a freshness comparison.

Track transaction phase from the first implementation: before COMMIT can be sent, failures cannot commit this explicit transaction; once COMMIT may have been sent, timeout/disconnection is uncertain unless server evidence proves rejection. Retain receipt IDs before committing. Once commit returns success, return the receipt even if the deadline has passed. Close failed/abandoned leases using SQLx's supported cleanup behavior; do not copy the existing autocommit writer's outcome classification unchanged.

- [ ] Add controlled-overlap tests for same-account first creation and different-account shared-subject updates in both old/new arrival orders. Verify complete final rows and identities. Add unknown-learner and failure-after-earlier-write rollback tests; isolate the intended failure. Run the focused contracts, review the transaction/constraint interaction, and commit with a WHY body.

### Task 3: Verify failure phases and complete delivery

**Interfaces:** Preserve Task 1's port and Task 2's implementation; test controls remain private to tests.

- [ ] Add real-backend tests for expired deadlines, exhausted pool, blocked statement timeout, cancellation and backend termination before COMMIT. Assert durable no-save behavior where provable, pool recovery and bounded completion. Test a deferred constraint rejection at COMMIT so earlier successful statements cannot masquerade as a save.
- [ ] Add a deterministic lost-COMMIT-response test: a small bounded test proxy lets PostgreSQL commit but withholds its acknowledgment from the client. Assert `CompletionUnknown`, the entire batch present, no automatic retry and eventual pool recovery. Confirm SQLx 0.9's commit exchange from its source/documentation before implementing the fault control; avoid sleeps as synchronization or a general PostgreSQL protocol framework.
- [ ] Verify known late completion preserves the receipt using a deterministic private completion/classification test alongside a real commit-success test. Caller-side deadline/output handling is excluded; do not add an unused application wrapper. Check error sanitization with injected provider detail and source data markers.
- [ ] Run the applicable [CI checks](../../../.github/workflows/ci.yml), including the full real PostgreSQL suite, core/app boundary enforcement and its contract tests for the dependency change. Review the whole diff for simpler equivalent code, precise names, scope and meaningful assertions. Update test documentation, commit with WHY, and obtain one independent final review before opening the implementation PR. Do not merge automatically.

## Handoff

This plan was self-reviewed against #109 and the merged repository: each acceptance condition has an owning task; transaction-only failures are distinguished from autocommit failures; no additional schema or import-workflow feature is assumed. No implementation has started. Review this plan before executing it in a fresh session; native execution is recommended because the tasks share one tightly coupled contract and transaction.
