# Storage agnostic vector selection plan

Unimplemented proposal accompanying the [request example](story-generation-request.yml).
The [architecture](../../ARCHITECTURE.md) and retained [SQL schema](schema.postgresql.sql)
govern this work. This note does not approve new APIs, schema changes or product
behavior. When SQL storage is implemented, reuse shared adapters and connections.

## Spaces and identity

The schema stores `embedding_profiles` and `entry_embeddings`, with one vector per
`(entry_id, embedding_profile_id)`. Use entry IDs with their existing subject and
collection relationships. Shared entry embeddings need not be learner-specific;
every query must preserve the authorized, eligible entry scope.

The proposal treats profiles as immutable. Provider, model, model revision,
dimensions and input format revision identify a profile in the schema. Query and
stored vectors must use compatible profiles; reject incompatible dimensions or
invalid values. Changes affecting embedding compatibility require a different profile.

The request example's `space` resolves an embedder, model, dimensions and metric.
Its mapping to an embedding profile, and representation of any additional provider
settings affecting compatibility, remain open design questions.

## Vector preparation

An explicit, bounded preparation/backfill operation would embed entries after imports
or profile changes. The schema's `input_hash` fingerprints the embedding input;
the profile identifies its input format revision. Selection does not silently
backfill stored embeddings. Story query embeddings use the same compatible profile.

Compare expected input hashes when reading vectors. Proposed policy: missing/stale
vectors sort last for similarity ranking and cannot pass distance filters. Report
missing coverage. With no usable vectors, report the unavailable operation; the
application's required/optional capability policy determines whether work can continue.
Do not silently ignore a required similarity constraint.

## Selection flow

Following the [boundaries](../../ARCHITECTURE.md#boundaries):

```text
application -> selector capability
selector -> injected embedder -> query vector
selector -> injected repository(resolved query data) -> selected entries
selector -> application -> next requested operation
```

The resolved query carries data, including vector/profile information, filters,
ordering and limits. The repository never receives an embedder or effectful callback.
Its adapter may combine metadata and vector filtering/ranking in one SQL query with
joins, ordering and limits; no separate vector repository or full distance list is required.

Read coherent learner data. For the proposed ordering, category preference remains
separate from filtering; earlier ordering rules take priority and entry IDs break
final ties. Apply limits only after preserving the agreed ordering and target rules.
Target/filter conflicts and missing-vector behavior need explicit contracts before
implementation. Pure domain decisions operate on supplied data without invoking ports.

## Responsibilities and next work

Composition roots construct dependencies; application owns the workflow and shared
execution policy. Selectors compose supplied ports within those limits. Core owns
port contracts and pure domain rules; extensions implement capabilities and storage.

An initial implementation could use exact calculations for one metric/precision
contract. Verify profile compatibility, scope, freshness, thresholds, ordering and
targets against each implemented adapter; test PostgreSQL guarantees on PostgreSQL.
Later approximate indexes need explicit provisioning and an accuracy contract;
[filtered approximate scans](https://github.com/pgvector/pgvector#filtering)
can miss qualifying results. Concrete interfaces, coherent reads across stores,
input freshness checks, backfill scheduling and index provisioning remain open.
