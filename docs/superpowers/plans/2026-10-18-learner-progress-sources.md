# Learner Progress Sources (#107) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Represent separate accounts as learner progress sources while preserving the collection-list command's behavior.

**Architecture:** Update the retained bootstrap schema, scope account uniqueness to learner/collection/account, and select distinct collection IDs in the existing storage adapter. Keep the application and query interface unchanged. Recreate databases when their schema changes; add no migration mechanism.

**Tech Stack:** Existing Rust 1.98.1 workspace, SQLx 0.9, Tokio, PostgreSQL 18+ and pgvector; no new dependencies or migration framework.

**Spec:** [#107](https://github.com/kalorz/yomibu/issues/107), the [agreed decisions in #97](https://github.com/kalorz/yomibu/issues/97#agreed-decisions), and [completed design outcome #106](https://github.com/kalorz/yomibu/issues/106#outcome). These ticket descriptions are the specification, amended by the user's instruction below.

**Scope amendment (2026-10-18):** The user made database recreation without migration mechanisms a general rule until further notice. The standing [schema-change policy](../../../ARCHITECTURE.md#schema-changes) supersedes the tickets' existing-database upgrade and data-preservation requirements. Omit upgrade scripts, legacy-schema fixtures and upgrade/rollback tests. The policy was recorded before implementation and governs this change; no existing database was rebuilt.

**Baseline:** GitHub main and the clean checkout are `8fa458a3f73ca0911653bfbe541700569a7fdc4d` (2026-10-18). Read [AGENTS.md](../../../AGENTS.md), [ARCHITECTURE.md](../../../ARCHITECTURE.md), [GLOSSARY.md](../../../GLOSSARY.md), the [retained schema](../../plan/schema.postgresql.sql), affected code/tests, README and CI. No consequential conflict was found. Recheck changed sources before execution.

## Global Constraints

- “Retain `learner_progress` and its other fields.” Keep identifier types/defaults and foreign-key behavior in the new schema; preserving data across schema changes is not required.
- “No separate membership or profile table is added.” Collection membership is derived from progress sources, including sources with no progress yet.
- “Record these definitions in `GLOSSARY.md` before code adopts them.” The ticket already supplies agreement; no new naming approval or separate glossary commit is needed.
- “Keep the operation's user-facing meaning as listing collections.” Preserve unspecified ordering, empty results, learner isolation, sanitized failures and execution limits.
- “Preserve retained plans; update necessary references without adopting unrelated proposals.” Generic observations and raw subject content must not be relabeled as learner progress.
- The retained schema requires “PostgreSQL 18+” and pgvector. Keep SQL and backend guarantees in `yomibu-ext`; preserve the existing core port and application ownership.
- Excluded: WaniKani HTTP/import/save workflows, credentials, account selection/merging, entry resolution, eligibility, generation, new membership/account/profile entities, any migration mechanism and other epic tasks.

## Review Focus

- A null account alongside two named accounts: permit all three, reject a second null or repeated named account only within the same learner/collection (Task 1, source identity test).
- Two sources for the same subject: updating one source's metadata/progress must leave the other source, another learner and shared content unchanged (Task 1, separation test).
- Collection-mismatched or missing parents: preserve composite foreign keys, source-delete cascade and subject/learner/collection delete restrictions (Task 1, reference test).
- Multiple sources with no progress: list each collection once and exclude unrelated collections (query contract).
- Unknown learners remain empty successes; database errors and deadline failures remain failures (existing query tests and CLI wiring).

## File Map

| Files | Responsibility |
| --- | --- |
| Modify `GLOSSARY.md`, `AGENTS.md` | Agreed definitions and standing terminology guidance. |
| Modify `docs/plan/schema.postgresql.sql` | Fresh database structure, constraints and comments. |
| Modify `crates/yomibu-ext/src/postgres.rs` | Distinct collection query and accurate adapter Rustdoc. |
| Modify `crates/yomibu-core/src/ports/infrastructure/mod.rs` | Describe source-derived collection membership beside the unchanged query interface. |
| Modify `crates/yomibu-ext/tests/postgres.rs` | Real PostgreSQL schema and query tests using the existing isolated-schema setup. |
| Modify `crates/yomibu-cli/tests/cli.rs` | Multiple-source fixture and unchanged output/error wiring. |
| Modify `README.md` | New table access prerequisite, unique collection results and existing database-recreation instructions. |
| Modify `ARCHITECTURE.md`, `docs/plan/jmdict-import-and-matching.md`, `docs/plan/grammar-import-and-matching.md`, `docs/plan/replay.md` | Only directly affected terminology/examples and schema references; preserve document statuses. |
| Modify `docs/plan/story-generation-request.yml` | Normalize the existing comment's “learner-progress sources” to the canonical spelling; no configuration changes. |

No production application, domain, CLI behavior, dependency or CI configuration change is expected. Existing CI already runs ignored PostgreSQL tests.

## Task 1: Adopt progress sources and preserve collection lookup

**Files:** All modified files in the map. No new implementation files are expected.

**Interfaces:**

- Consumes the retained `learners`, `collections`, `subjects` and `learner_progress` structure and the existing PostgreSQL fixture helpers.
- Preserves `LearnerCollectionQuery::collection_ids_for_learner(&self, learner_id: LearnerId, deadline: Instant) -> Pin<Box<dyn Future<Output = Result<Vec<CollectionId>, StorageError>> + '_>>`, `PostgresLearnerStorage::new(pool: PgPool) -> Self`, and `ListCollectionIds::execute`.
- Produces `learner_progress_sources` with all former association columns, `learner_progress.learner_progress_source_id`, and the constraint names below. Only fresh databases need to support this schema.

- [x] **1. Record the agreed language before changing SQL or Rust.** Replace the glossary's learner-progress definition, add learner progress source, and remove the separate Source observations row. Use #107's definitions:

  - **Learner progress source:** identifies where progress for one learner and collection comes from—an external account or manual input. Collection membership is derived from progress sources; a source can exist before progress arrives.
  - **Learner progress:** saved information about a subject attributed to a learner progress source, including stage, learning dates, manual declarations and exclusions.

  Retain the statement that imported progress does not prove mastery as explanatory prose. Extend AGENTS' existing Terminology section with these passages, preserving its agreement rules:

  > For each ticket, identify the affected domain terms and check them against GLOSSARY.md and accepted ticket/epic decisions before editing. Before completion, check terminology across the touched schema, code/contracts, tests and documentation, including the translation of provider terms. Reuse canonical terms; raise unresolved naming or meaning conflicts instead of introducing synonyms. Keep the check scoped to the affected work.

  > Qualify external domain terms with their provider or system when ambiguity is possible. A namespace counts as qualification. Translate external concepts into Yomibu's agreed terminology at the integration boundary.

  Include the ticket's examples: **WaniKani assignment**, `wanikani::Assignment` or `WaniKaniAssignment`, then **learner progress** after translation; avoid redundant prefixes within provider namespaces and preserve original API fields/payloads.

- [x] **2. Write PostgreSQL regression tests.** Add separate ignored `#[tokio::test]` functions in `crates/yomibu-ext/tests/postgres.rs`, using `with_postgres`. Use fixed IDs and synthetic payloads: Alice, Bob, two collections, Alice's accounts A/B, a null-account source, and one shared 食べる subject.

  | Test | Assertions |
  | --- | --- |
  | `postgres_progress_source_identity` | Insert Alice/A, Alice/B and Alice/null for one collection: `assert_eq!(count, 3)`. Duplicate A and duplicate null each fail with SQLSTATE `23505`; the same account ID and a null account succeed for another learner and for another collection. Test both INSERT and UPDATE attempts to collide with an existing account. |
  | `postgres_progress_is_source_scoped` | Store Alice/A stage `"5"`, Alice/B stage `"1"`, and Bob's progress against one shared subject. Update A to `"7"` and change A's account/access JSON. Assert A's values changed, B/Bob's complete rows and metadata are equal to their before snapshots, and the subject ID/content is unchanged. A duplicate source/subject progress row fails with `23505`. |
  | `postgres_progress_source_references` | Missing learner/collection/source/subject and mismatched collection references fail with `23503`. Deleting a referenced learner, collection or subject fails with `23001` (PostgreSQL 18's RESTRICT violation); deleting Alice/A cascades only its progress, preserving B/Bob and the shared subject. |

  Compare rows through SQL JSON/text projections using existing SQLx features. For rejections use `assert_eq!(error.as_database_error().unwrap().code().as_deref(), Some("23505"))` (or the reference/restriction code above). Use autocommit or savepoints so expected failures do not poison subsequent checks.

- [x] **3. Extend the existing query and CLI regressions.** In `check_query_contract`, replace association inserts with progress sources and add two named accounts to Alice's existing null-account/shared-collection source, without adding progress rows. Preserve the existing expected sorted IDs `[0x10, 0x20]` for Alice and `[0x10, 0x30]` for Bob, unknown/unassociated empty results, ownership-only exclusion and every deadline/error check. Rename lock targets. Keep duplicate-source rejection in the source identity test. In `commands_use_postgres`, use the same multi-source condition and renamed failure-injection table (`hidden_progress_sources`); retain assertions for text/JSON UUIDs, newlines, exit status and sanitized errors. Add the missing real-command unknown-learner JSON assertion `assert_eq!(success(&output), "[]\n")`; do not repeat the SQL integrity matrix at the CLI layer.

- [x] **4. Run the regressions and confirm the expected failure.** With the README's disposable PostgreSQL 18+ test database and public vector extension configured, run:

  ```sh
  cargo test --locked -p yomibu-ext --test postgres -- --ignored
  cargo test --locked -p yomibu-cli --test cli commands_use_postgres -- --ignored
  ```

  Expect missing-table failures before implementation; after the schema change, query tests must still catch old-table lookup or duplicate IDs until the adapter changes. Missing prerequisites do not count as the intended failure.

- [x] **5. Update the bootstrap schema.** Rename the table and progress FK column; keep every other column/type/default and FK action. Replace the old learner/collection unique constraint with `CONSTRAINT learner_progress_sources_account_key UNIQUE NULLS NOT DISTINCT (learner_id, collection_id, external_account_id)`. Preserve `UNIQUE (id, collection_id)` and the composite progress FKs. Use these names in the bootstrap schema:

  | Baseline object | New object / action |
  | --- | --- |
  | `learner_collections_pkey` | `learner_progress_sources_pkey` |
  | `learner_collections_collection_id_fkey` | `learner_progress_sources_collection_id_fkey` |
  | `learner_collections_learner_id_fkey` | `learner_progress_sources_learner_id_fkey` |
  | `learner_collections_learner_id_collection_id_key` | Replace with `learner_progress_sources_account_key` as above. |
  | `learner_collections_id_collection_id_key` | `learner_progress_sources_id_collection_id_key` |
  | `learner_collections_collection` | `learner_progress_sources_collection` |
  | `learner_progress_learner_collection_id_collection_id_fkey` | `learner_progress_source_collection_fkey` (explicitly name this constraint in the bootstrap). |

  Keep `learner_progress_pkey` on `(learner_progress_source_id, subject_id)`, its subject FK and `learner_progress_subject` index. Update comments for source-derived membership and source-scoped metadata/progress; preserve each `source_data` column's owning-table meaning.

- [x] **6. Update the adapter and contract.** Use `SELECT DISTINCT collection_id FROM learner_progress_sources WHERE learner_id = $1`; preserve binding, deadlines, errors and cleanup. Update adapter Rustdoc and the core contract's storage reference and membership explanation. No query rename, new port or application deduplication is needed.

- [x] **7. Reconcile directly affected documentation.** Update README's read permission/table reference and explain that multiple sources still produce one collection ID; creating a learner creates no progress sources. Retain its link to the standing schema-change policy and existing database-reset instructions. In architecture examples translate WaniKani assignments into learner progress, and use learner progress where examples mean saved learner state. In JMdict/grammar references update table/column names and progress ownership; distinguish subject content from progress. In replay references use learner progress instead of the removed separate model term. Retain ordinary evidence language, unrelated observations, collection/subject/entry distinctions and the proposals' existing status. Keep CLI help's collection-facing language.

- [x] **8. Verify fresh-schema behavior and run affected CI checks.** Rerun Step 4; the existing database fixtures apply the complete bootstrap in fresh isolated schemas. Expect every new and existing contract/wiring test to pass. Use README prerequisites and the existing pinned tools (cargo-bylaw 0.1.0, cargo-machete 0.9.2); do not silently substitute a backend or mark unavailable checks as passes:

  ```sh
  cargo fmt --check
  cargo clippy --locked --all-targets --all-features -- -D warnings -D clippy::redundant_clone -D clippy::unused_async
  cargo test --locked --all
  cargo test --locked --all -- --ignored
  RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
  python3 -B tools/check_architecture.py
  python3 -B -m unittest discover -s tools -p 'test_*.py' -v
  python3 -B tools/check_domain.py
  cargo machete --with-metadata --skip-target-dir
  git ls-files --error-unmatch Cargo.lock
  git diff --exit-code -- Cargo.lock
  git diff --check
  ```

  Set `YOMIBU_TEST_DATABASE_URL` for the ignored suite. Expect zero exits, passing database tests, no warnings and no lockfile changes. Report local platform coverage and unavailable prerequisites; CI retains Linux/macOS quality coverage.

- [x] **9. Perform the final scoped review and commit.** Search touched schema/code/tests/docs for `learner_collections`, `learner_collection_id`, `Source observations`, `source observations` and unqualified provider terms. Old SQL names are legitimate only in explicit historical explanations such as this plan. Check glossary meaning, raw payload preservation and retained-plan status manually; no broad text replacement. Review the complete diff for unnecessary machinery and preserved boundaries, rerun affected checks if review changes code, and commit as `feat: represent learner progress sources and list unique collections`. Report additions/deletions/new files, verified guarantees and any remaining verification limits.

## Self-review and handoff

Self-reviewed against #107 and relevant #97/#106 decisions, with the user's standing schema-change policy. The single task covers terminology/guidance, schema/account separation, reference integrity and unique collection results. Removed all migration work and checked the remaining paths, interfaces and verification. The user approved native implementation. All plan steps are implemented and local CI checks pass: 31 Rust tests, 8 PostgreSQL/CLI tests and 24 architecture-enforcement tests. Formatting, Clippy, Rustdoc, dependency checks and lockfile/whitespace checks pass on Linux. Final independent review of the implementation found no defects or deferred concerns. No existing database was rebuilt.
