# Storage agnostic vector selection plan

Proposed design for the [story request](story-generation-request.yml).
Extensions produce vectors and selection rules. Shared repository ports isolate
storage details, avoiding a backend adapter for every extension.

## Vector spaces

Use separate rows per purpose, rather than extension-specific vocabulary columns:

```text
vector_spaces(id, owner, purpose, producer_revision, dimensions, metric)
vocabulary_vectors(vocabulary_id, space_id, input_revision, vector)
UNIQUE(vocabulary_id, space_id)
```

A space pins its query embedder and producer settings. Producer changes create
a new immutable space profile. Reject incompatible dimensions and invalid vectors;
stale vectors count as missing. A namespace identifies ownership, not authorization.
Reuse the application's database and storage adapters.

## Selection flow

1. Read coherent learner data and derive the eligible inventory.
2. Apply metadata filters through typed query expressions: comparisons, membership,
   array containment, and explicit category ordering. Backend adapters interpret
   them; extensions never supply SQL. Add operations only for demonstrated needs.
3. Resolve the space and embed the requested story outline once. Query and stored
   vectors must use the same profile.
4. Obtain distances for authorized, filtered IDs and expected input revisions. A distance
   threshold excludes missing vectors; ranking places them last for that rule.
5. Combine ordering rules, break ties by vocabulary ID, then apply final caps.
   Reserve target slots; target conflicts fail explicitly.

Example:

```ruby
space = repositories.vectors.for_space("extension-x/topic-v1")
query = embedders.for_space(space.id).embed(story_description)

distances = space.distances(
  query: query, within: filtered_ids, revisions: input_revisions
)
selected = selection.finish(
  ids: filtered_ids,
  distances: distances,
  order_by: [category_preference, distance_ascending],
  targets: targets,
  limit: 20
)

# Only when similarity narrowing is explicitly requested:
neighbors = space.nearest(
  query: query, within: filtered_ids, revisions: input_revisions,
  max_distance: threshold, limit: 20
)
```

Distance reads support combined ranking. Use `nearest` only for explicit
threshold/top-k narrowing; never truncate before another primary order.

## Adapters and implementation

Core owns typed query/result contracts. The application owns scope, composition
and budgets. Extensions implement embedders and selection capabilities;
`PostgresVectorRepository` and `InMemoryVectorRepository` implement shared storage
ports. Follow the [architecture guide](../../AGENTS.md#architecture).

Start with exact distances. PostgreSQL uses pgvector; in-memory storage uses the
same metric, precision and error/missing-value rules. Approximate search needs an
explicit accuracy contract: filtered approximate scans can miss qualifying results.
See [pgvector filtering](https://github.com/pgvector/pgvector#filtering).

Implement one metric/use case first. Add shared adapter-contract
checks for profile validation, scope, missing/stale vectors, thresholds, ties and
targets; test PostgreSQL on PostgreSQL. Provision per-space indexes in migrations
as needed; pgvector's [dimension-specific indexes](https://github.com/pgvector/pgvector#can-i-store-vectors-with-different-dimensions-in-the-same-column)
do not require new extension-specific Rust adapters. Result types, snapshot
coordination, freshness and approximate-search limits remain implementation decisions.
