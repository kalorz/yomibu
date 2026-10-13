# Storage agnostic vector selection plan

Proposed design for the [story request](story-generation-request.yml), not an
implemented API. PostgreSQL is a future backend; when SQL storage exists, reuse
that database and shared adapters across extensions.

## Spaces and identity

Illustrative storage layout:

```text
vector_spaces(id, namespace, purpose, producer_profile, metric)
vocabulary_vectors(material_key, space_id, input_revision, vector)
UNIQUE(material_key, space_id)
```

An immutable producer profile identifies the embedder/model, settings, input recipe
version, and dimensions. Producer changes create a new profile/space. Query vectors
must match the stored profile; reject incompatible dimensions or invalid values.

Material keys identify catalog entries or source-collection/entry pairs, not bare
inventory strings. Shared vocabulary vectors need not be learner-specific. Every
query uses authorized eligible keys; namespaces identify ownership, not permission.

## Vector preparation

An explicit, bounded preparation/backfill operation embeds vocabulary after imports
or profile changes. `input_revision` fingerprints the exact embedding input;
the profile pins its transformation recipe. Generation does not silently backfill.
It embeds the story outline once with the same profile.

Compare expected input revisions when reading vectors. Missing/stale vectors sort
last for similarity ranking and cannot pass distance filters. Report missing
coverage; if no usable vectors exist, return a setup issue rather than silently
ignoring similarity.

## Selection flow

Read coherent learner data. Apply metadata predicates through shared typed query
ports: comparisons, membership and array containment. Category preference is an
ordering rule, separate from filtering. Metadata/vector queries may run separately.

Prepare distance evidence, then run synchronous filtering/ranking steps through
core's existing `select_target_first`. It retains targets and applies final caps;
filter conflicts fail explicitly. Never truncate by similarity before another
primary order or target reservation. Break final ties by material identity.

Illustrative Ruby-like pseudocode; preparation APIs are provisional:

```ruby
distances = await vectors.distances(
  space: profile.id, entries: scoped_entries, query: query_vector
)
steps = prepare_steps(metadata, distances, threshold, order_by)
selection = select_target_first(selection_input, steps)
```

## Responsibilities and next work

The [composition root](../../AGENTS.md#dependencies-and-composition) constructs
dependencies. The application coordinates async I/O, scope and budgets.
`SelectionStep` stays synchronous. Core owns query/result/error contracts. PostgreSQL and in-memory
adapters share those contracts; extensions implement producers and capabilities.

Start with exact calculations for one metric/precision contract. Test both adapters
for profile, scope, freshness, thresholds, ties and targets; test PostgreSQL
on PostgreSQL.
Ordinary indexes support initial queries. Space creation is data, not a migration.
Later approximate indexes need explicit provisioning and an accuracy contract;
[filtered approximate scans](https://github.com/pgvector/pgvector#filtering)
can miss qualifying results. Concrete core types, snapshot coordination,
backfill scheduling and index provisioning remain open.
