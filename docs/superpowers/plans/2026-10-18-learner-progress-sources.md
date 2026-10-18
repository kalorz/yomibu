# Learner Progress Sources (#107) Implementation Plan

Status: implemented. This is the user-requested implementation plan; the agreed
[#107](https://github.com/kalorz/yomibu/issues/107),
[#97](https://github.com/kalorz/yomibu/issues/97) and
[#106](https://github.com/kalorz/yomibu/issues/106) descriptions remain the specification.

Scope: support separate account progress for one learner and collection, preserving
unique collection lookup and existing interfaces. No import workflow, account
selection, progress merging or other epic work is included.

The user's standing [schema-change policy](../../../ARCHITECTURE.md#schema-changes)
replaces the original upgrade requirement: recreate databases when schemas change;
do not add migrations or existing-data upgrade tests until that policy is revised.

## Implementation

- [x] Record the agreed learner progress source and learner progress definitions in
  [GLOSSARY.md](../../../GLOSSARY.md), and the ticket's terminology checks and provider
  qualification rules in [AGENTS.md](../../../AGENTS.md).
- [x] Rename the table and progress reference in the
  [retained schema](../../plan/schema.postgresql.sql). Enforce account uniqueness,
  including a single null account per learner/collection; retain source-scoped
  metadata/progress, composite foreign keys and deletion rules.
- [x] Return distinct collection IDs from progress sources in the PostgreSQL adapter.
  Preserve the query interface, source-derived membership before progress arrives,
  learner isolation, empty results, failures and deadlines.
- [x] Update directly affected contracts, README, CLI help and retained references
  using the agreed terminology without adopting unrelated proposals.
- [x] Add real PostgreSQL regressions for account/null collisions, independent progress
  and metadata, references and deletion rules. Isolate the source-to-collection delete
  restriction with a collection that has a source but no subjects. Extend query/CLI
  fixtures to catch duplicate collection IDs while preserving existing failure checks.
- [x] Run affected checks from [CI](../../../.github/workflows/ci.yml), review the diff
  for terminology, scope and unnecessary machinery, and obtain independent review.

Validation commands and tool versions belong to CI; local database prerequisites
belong to [README.md](../../../README.md). Current results and review follow-up are
recorded in [PR #108](https://github.com/kalorz/yomibu/pull/108).
