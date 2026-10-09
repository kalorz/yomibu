# Yomibu specification

This records product intent and constraints. Use [the code map](ARCHITECTURE.md#current-module-map)
for implementation and [PLAN](PLAN.md) for open work. Follow the documentation
rules in [AGENTS](AGENTS.md). Future direction does not authorize implementation.

## Current contracts

| Task | Reference |
| --- | --- |
| Sync and offline status | [Sync rules](#wanikani-synchronization), [usage](docs/SYNC.md) |
| Analysis and evaluation | [Check boundaries](#bounded-analysis-and-evaluation), [input/output](docs/ANALYSIS.md) |
| Learner inventory and story generation | [Story contract](docs/STORY_GENERATION.md) |
| Embeddings and vector caches | [Retrieval contract and evidence](docs/RETRIEVAL.md) |
| Dictionary installation and loading | [Dictionary contract](docs/DICTIONARY.md) |

Current commands are in [README](README.md#current-commands). They do not select
accepted exercises. [The code map](ARCHITECTURE.md#current-module-map) locates
implementation and tests; [the glossary](docs/GLOSSARY.md) defines terms.

## Product

Yomibu aims to provide useful Japanese reading practice within the current
experimental limits. Choose the simplest design that solves the product problem.

The first interface is a CLI. Domain behavior and integrations must also be
callable through a library without starting a CLI process. Later interfaces may
include web applications, native applications, and an MCP server.

### Practice direction (future)

1. Combine WaniKani learner state with Yomibu-owned grammar knowledge, initially
   entered through a local file.
2. Accept explicit practice targets; later select targets using WaniKani
   difficulty and Yomibu mistakes.
3. Retrieve structured lexical information and examples for the targets.
4. Generate two short, coherent candidate passages from the prepared constraints.
   Prefer reliable offline construction where it meets the reading-quality needs;
   model generation is a bounded, explicitly authorized component. Current story
   generation uses one experimental provider call without retry.
5. Analyze candidates, run explicit constraint checks, and include a minimal
   naturalness/coherence review in the first usable generation milestone. Select
   an acceptable candidate and use bounded repair only when none are acceptable.
6. Later extend grounded criticism and reading/translation quizzes. Focused
   checks of intended sense and contextual reading are required as soon as
   ambiguous targets are supported.
7. Preserve attempts and mistakes for future target selection.

The pedagogical goal is zero unknown kanji and vocabulary, deliberate practice of
intended vocabulary/readings, and practical adherence to known grammar. Yomibu
practises learned material in fresh contexts; it is not primarily a vocabulary
introduction tool.

Simple sentences and short stories are first-class outputs. Passage length and
grammatical complexity are controlled separately from allowed vocabulary. More
complex material must still respect learner constraints. Knowing each kanji in a
compound does not establish knowledge of the word; writing an unknown word in
kana does not make it acceptable. If no candidate satisfies the constraints,
report failure rather than quietly introducing unfamiliar material.

Keep the meaning of "known" revisable. Preserve the source
learning state so policy can evolve independently. Later validation must address
inflections, particles, lexical ambiguity, and words absent from WaniKani.
Membership checks alone cannot establish complete linguistic correctness.

The primary learner outcome is **"WOW! I CAN READ JAPANESE!!"**: understanding
meaningful Japanese built from familiar language. Evaluate comprehension,
reading friction and enjoyment with learner feedback, not a validator score.
Prefer reliable offline work in preparation, generation, validation and narrowly
defined repair; AI is one component, not the authority for permissions or
correctness. This direction does not authorize new repair or acceptance behavior
in the current experimental story path.

Separate mandatory constraints from quality ranking: a high soft score cannot
compensate for a failed mandatory check. A completed check returns pass, fail, or
inconclusive; execution errors and checks not run are separate states. Missing
analysis is not evidence of correctness. An inconclusive mandatory check must be
resolved through an explicitly supported assessment or prevent acceptance, never
silently pass.

### Intended readings and senses

The following is the intended validation/quiz contract. Current inputs retain
reading and sense evidence, but bounded checks do not establish contextual correctness.

A practice target identifies the word, intended reading, and intended sense.
Retrieve supporting usage information before generation, then check whether the
generated occurrence supports that use. Selecting a reading and meaning in
advance constrains generation but does not prove the output is correct. The
generator's claimed reading is not independent validation.

When ambiguous targets are first supported, include a focused, grounded check of
their sense and reading; do not defer this until the broader critic milestone.
Rephrase or reject ambiguous reading-quiz prompts. Do not mark a contextually
valid alternative wrong merely because it differs from the generation target.
Deterministic answer normalization and comparison operate on contextually
validated acceptable readings; they do not establish those readings themselves.

Reading quizzes will compare kana or romaji answers after deterministic
normalization. Translation answers may be in any language and will be evaluated
for preserved meaning by an LLM semantic judge, not against a single canonical
translation. Do not build a Yomibu SRS initially.

### Data responsibilities and deferred capabilities

| Source | Responsibility |
| --- | --- |
| WaniKani | Source learner progress, lexical information, review statistics, and context sentences |
| Yomibu history | Attempts and mistakes made in Yomibu |
| Yomibu grammar knowledge | Learner data, initially entered through a local file |
| JMdict | Optional enrichment for useful reading/sense distinctions, not a mandatory lookup |
| Trusted external sentence corpora | Optional later grounding when their value is demonstrated |

Grammar knowledge is evolving learner data, distinct from application preferences
such as desired passage length. Its initial file is an input and persistence
mechanism, not a permanent domain boundary. When a database is introduced, store
grammar knowledge alongside vocabulary progress and attempt history; files may
remain optional import/export rather than a competing authoritative copy.
Future grammar integrations retain provider-scoped identifiers and source
descriptions. Entries from different providers remain independent, even when
their descriptions look similar; do not require a canonical grammar catalog or
infer equivalence between Bunpro and Renshuu. Manual declarations use
user-provided descriptions and automatically assigned technical identifiers.
Do not implement those integrations or their schemas now.

Retrieval starts with structured lookups. RAG does not require vector search.
Embeddings become appropriate only for a demonstrated retrieval problem, such as
ranking a large sentence corpus or selecting semantically useful known words.

Quizzes, vector databases, PostgreSQL, Bunpro, JMdict
and web interfaces remain deferred. PostgreSQL is the likely later web database;
pgvector remains conditional on a demonstrated vector-search need. Do not add
placeholder implementations for these future capabilities.

## Historical capability boundaries

Retired commands, research protocols and code pins are in [history](docs/history/README.md).
Their original contracts describe those revisions, not current behavior.

## Bounded analysis and evaluation

`analysis::Sentence` retains unchanged nonblank text of at most 100 Unicode scalar
values. The pinned Sudachi adapter supplies C-mode whole units and A-mode components
with original UTF-8 byte spans. Familiar components/kanji cannot authorize a whole
word. Other grammar, multiword expressions and contextual reading/sense expose
limitations; there is no general Japanese validator.

Dictionary pins, configuration, developer setup and loading rules have one
home: [dictionary contracts](docs/DICTIONARY.md). No ambient configuration, fallback
tokenizer or runtime download is allowed.

`japanese_constraint_checks::evaluate` synchronously takes concrete analysis,
unchanged free-form declarations, and explicit word/rule bindings. Vocabulary tuples
retain written form, dictionary-style katakana reading, nonblank sense and `direct_object` boolean.
True supplies transitive-use evidence for that tuple; false supplies no positive
object-use evidence, not a claim of intransitivity. Such metadata needs source
evidence and is never inferred from を. A sense label is not contextual validation.
Unknown identities, competing supplied tuples, reading mismatches and unresolved
lexical/POS alternatives affecting a check remain Inconclusive. Dictionary entry
grouping cannot settle them; ordinary polysemy alone does not invalidate the
bounded identity check.

Recognition covers two single-clause patterns: optional nominal topic with
nominal です, or optional topic/direct object with a regular godan/ichidan polite
verb in present/past/negative forms. A nominal slot is one whole lexical unit,
possibly a compound. At most one final 。 is optional. Other punctuation,
whitespace, clauses/modifiers, questions and unsupported morphology expose scope
limits. The seven `GrammarRule` variants require explicit bindings to existing
declaration IDs; descriptions are never parsed. Malformed bindings/analysis are
typed errors. [Analysis usage](docs/ANALYSIS.md#input-and-supported-checks) lists the rules.

The five checks cover vocabulary, inflection, particles, grammar and structural
scope. Scope independently assesses coverage/applicability, not permission
outcomes; all original text must be accounted for before Scope can Pass.
Applicability remains unresolved for unrecognized constructions. Supported
permission failures retain their Fail reason/span beside other uncertainty.
Keep completed Pass/Fail/Inconclusive, execution errors and NotRun distinct.
Aggregate precedence is Fail, NotRun, Inconclusive, then Pass.

A transitive-use binding cannot resolve an object/predicate combination. Recognized
object + を + regular polite verb therefore retains Inconclusive for Particles
and Scope when permissions are present, even with `direct_object: true`.
Existing permission failures still determine Fail and retain decisive spans.
The uncertainty finding spans object through predicate, excluding a preceding
topic and optional final 。. Per-word vocabulary/morphology remain observable.
This deliberately limits ordinary object sentences too; it introduces no idiom
blacklist, contextual evidence, phrase exceptions or broader grammar support.

Every report exposes naturalness, multiword expressions and contextual reading/sense
as unassessed. A Pass never establishes an accepted exercise. The completed A1
investigation retains its frozen **no-go**, and the separate object-combination
safeguard does not revise that score. Research protocols, historical packet-only
contracts, review lineage and private scoring receipts are indexed in
[history](docs/history/README.md#analysis-investigation). Engineering tests are not
independent linguistic ground truth. Frozen/held-out references are not development
inputs; private learner material and undisclosed reference keys stay outside Git
and runtime composition.

## Offline analysis CLI

[Analysis](docs/ANALYSIS.md) defines input, output and execution behavior.
The CLI exposes the bounded checks above; it adds no exercise acceptance decision.

## Shared learner-inventory story path

[Story generation](docs/STORY_GENERATION.md) owns the inventory/request formats,
planning and assessment contracts, provider limits, and CLI behavior.
[Retrieval](docs/RETRIEVAL.md) owns embedding/cache contracts and measured evidence.
Manual and WaniKani material use this same path. The application handles optional
auto-sync; the story guide defines cache and first-run policies.

## Managed dictionary loading

[Dictionary contracts](docs/DICTIONARY.md) own installation, verification, update
and loading rules. Managed mapping requires verified bytes to remain unchanged
for the analyzer's lifetime. A receipt or read-only mode cannot prove that condition.

## Design vocabulary and composition

Name implemented responsibilities consistently:

- `Store` accesses stored data; its contract specifies writes, consistency,
  durability and failures. `Search` retrieves by criteria. `Client` communicates
  with a service. `Policy` makes deterministic decisions.
- Use `InMemory...` for volatile adapters and provider/backend names for others.
- Source progress is evidence; `LearnerKnowledgePolicy` derives eligibility.
  Inventory is the complete allowed material; selection is only the prompt subset.
- Prompt templates are reusable content. `openai_story_generation::PreparedRequest`
  is one finalized Responses payload.
  An exercise generator would own acceptance; current story generation does not.

Future responsibility names are not reserved APIs. Add types and extension points
when a caller needs them, rather than maintaining a speculative type catalog.

For future validated generation:

- Candidates are immutable. Analysis and findings refer to their exact text/spans.
  Edits create new candidates with full reassessment. A story's paragraphs and
  sentences are text structure; tokens and grammar hypotheses belong to analysis.
- Check titles and passages at their own scopes. Required checks gate acceptance;
  quality scores rank eligible candidates. Unresolved required checks prevent acceptance.
- Combine compatible model tasks into at most one invocation per generation,
  review or repair phase. Bound total calls, retries, repairs, candidates, time and
  cost across the operation. Oversized/incompatible requests fail explicitly.
  Do not retry after uncertain completion or hide calls inside checks.
- Typed responses do not prove linguistic correctness. Checks combined in one
  model call are not independent judges; do not invent calibrated confidence.
- Prompt content may be replaced and versioned independently of typed task
  inputs/outputs. Retain the prompt-set version used for a result.

### Data flow and storage boundaries

- Keep settings, learner/source data, and exercise requests separate. Callers supply
  credentials; the application resolves resources. Construction does not start work.
- Calls select an explicit learner/source scope; shared objects have no mutable
  current user. A source connection identifies one provider account/input.
  The current WaniKani learner ID must not become a multi-source Yomibu identity.
- Retain source provenance. Publishing one connection's refresh must not replace
  another's data or manual input. Material and progress remain separate concerns,
  even in one database. Read a coherent version for each generation.
- Derive knowledge on demand. If a materialized view becomes necessary, identify
  its input and policy versions; it must not become a second authority for progress.
- Pass existing values directly when storage is unnecessary. File and memory
  adapters have different persistence promises. Missing dependencies must fail
  explicitly rather than substitute model answers or discard writes.
- Future SQL/network stores need async contracts or bounded blocking execution.
  Release database transactions before model calls. Keep long-lived clients/stores
  shared, with scope, inputs, budget and results owned by each request.
- Cloud authenticates scope before library calls. Result persistence and idempotency
  need explicit contracts. A remote Yomibu client calls use cases, not a model adapter.
- Keep code public and credentials/learner data private. Private prompt sets stay
  private from creation. Public adapters do not imply rights to redistribute content.
  Resolve WaniKani licensing before a Cloud launch; see [content restrictions](https://docs.api.wanikani.com/20170710/#respecting-subscription-restrictions)
  and [terms](https://www.wanikani.com/terms).
- Start with existing modules; new crates or repositories need an actual responsibility.
  In-process extension contracts do not provide a sandbox.

## WaniKani synchronization

Supported platforms are macOS and Linux. Use one account per data directory and
full refresh synchronization.

### CLI and local files

```sh
YOMIBU_WANIKANI_SOURCE_API_KEY=... yomibu sync
yomibu status
```

Both accept a global `--data-dir PATH`. The default is `$HOME/.yomibu`; if HOME is
unavailable or empty, require an explicit directory. API-key flags override prefixed
environment bindings. The macOS CLI may save credentials in Keychain. Never store
secret values in configuration files; see [credential setup](docs/STORY_GENERATION.md#configuration-and-optional-work).

```text
~/.yomibu/
  wanikani.json
  wanikani.json.lock
```

`wanikani.json.lock` is a separate advisory lock file; its existence alone does not
indicate a running sync. Optional `config.toml` holds non-secret application settings;
see [story configuration](docs/STORY_GENERATION.md#configuration-and-optional-work).

### Synchronization behavior

- Retrieve `/user`, `/assignments`, `/review_statistics`, and relevant `/subjects`.
- Include kanji, vocabulary, and kana-only vocabulary referenced by assignments
  or review statistics. Exclude radicals. Do not filter by a proposed learning
  threshold, including whether a lesson has started or an item is burned.
- Collect the referenced subject IDs, fetch them in bounded batches, and follow
  each collection's pagination through its explicit termination.
- Preserve learner progress separately from lexical content. Respect the
  profile's content-access limit when retaining subject content, and report
  progress records whose lexical content is excluded. This exclusion must be
  distinguishable from an unexpected missing requested subject.
- Normalize and validate the complete retrieval before replacing the cache.
  Reject unexpected missing requested subjects, conflicting duplicates,
  mismatched subject types, and invalid required data. Accept documented nulls
  and tolerate additional response fields.
- Store synchronization start and completion times. `WaniKaniSyncData` contains
  observations collected during that interval; it is not a transactionally
  consistent snapshot of the remote system.
- A complete refresh replaces prior records rather than merging them, so removed
  or reset state does not survive through stale local records.
- A different account must not replace an existing account's cache. Return an
  actionable error directing the user to another data directory.
- Do not request individual review history, study materials, or the remote summary
  endpoint. Status is derived locally.

The API contract verified on 2026-09-27 is WaniKani v2, revision `20170710`, using
bearer authentication over HTTPS. Send the revision header explicitly. Collection
responses supply pagination URLs; rate-limit headers describe remaining capacity
and reset time. The documented limit is 60 requests per minute. See the
[official API reference](https://docs.api.wanikani.com/20170710/).

### Retained information

Normalize external DTOs into Yomibu-owned structures containing:

- Stable learner identity, username, level, vacation state, and relevant
  subscription/access metadata.
- Subject identity, kind, level, characters, meanings, readings, answer flags,
  vocabulary parts of speech, context sentences, hidden state, and SRS-system
  identifier.
- Assignment identity, subject reference, raw SRS stage, lifecycle timestamps,
  and hidden state.
- Review-statistic identity, subject reference, reading/meaning counters and
  streaks, reported accuracy, and hidden state.
- Resource update timestamps and a versioned cache envelope.

Do not retain secrets, raw response envelopes, audio, mnemonics, or unrelated
profile preferences. Missing review statistics remain absent rather than becoming
invented zero-valued records. Kana-only vocabulary does not receive fabricated API
reading records.

Use explicit subject variants where their fields differ. Preserve SRS stages as
source state, not as a pedagogical enum implying knowledge. Use parsed UTC
timestamps. Open-ended source classifications such as parts of speech do not
require a closed enum simply because they are strings.

### Offline status

Read and validate only the local cache. Do not access the network, read the token,
or construct an HTTP client.

Display:

- WaniKani username and level.
- Synchronization timestamp.
- Synchronized kanji and vocabulary counts, with kana-only vocabulary identified.
- Hidden and unavailable-content counts separately.
- Raw SRS-stage distribution, grouped by SRS system where necessary; do not invent
  a system association for progress lacking lexical content.
- Reading and meaning accuracy calculated from aggregate counters. Show
  "no reviews" when the corresponding denominator is zero.

Label these as cached observations, never as a count of "known" material. Do not
average per-subject percentages to calculate aggregate accuracy. Missing,
corrupt, or unsupported caches produce actionable errors and a nonzero exit
status. An empty but valid account succeeds.

### Schema 1 and status count semantics

The cache envelope is `{ "schema_version": 1, "snapshot": { ... } }`.
The persisted `snapshot` key is retained for schema-1 compatibility; it does not
promise an instantaneous view of the remote system. `WaniKaniSyncData` contains
the synchronization interval, learner, subjects, assignments, review statistics,
and `unavailable_subjects`. Subject lexical content is tagged
as `kanji`, `vocabulary`, or `kana_vocabulary`; only the first two have readings.
The cache contains normalized Yomibu data, not WaniKani response envelopes. See
[source.rs](crates/yomibu-core/src/domain/source.rs) for fields and
[fixtures](tests/fixtures/) for synthetic examples. Null dates remain null;
missing statistics remain absent records. Additional JSON fields are tolerated.

An unavailable-subject marker contains an ID and kind and means content was
excluded by the account's access limit. It does not carry invented lexical data
or an SRS-system association. Every retained subject or exclusion must be
referenced by progress. References must resolve with matching kinds. Normalized
caches reject duplicate resource IDs, multiple assignments/statistics for one
subject, and overlapping available/excluded content. At the HTTP normalization boundary,
identical retained records (including update timestamps) collapse by resource ID;
conflicting duplicates are rejected. Differences in ignored source fields do not
create a conflict. Other validation includes
required text, positive IDs, source level/access bounds, valid timestamp shapes,
an ordered synchronization interval, and reported percentages within 0–100.

- Kanji/vocabulary counts count retained lexical subjects, including hidden,
  unstarted, and burned items. Vocabulary includes kana-only vocabulary and
  displays that subset separately.
- Hidden count is the number of distinct subject IDs flagged hidden in any
  retained subject, assignment, or statistic. Unavailable content counts distinct
  explicit exclusions. These counts may overlap and are not additive partitions.
- SRS distribution counts assignments, preserving raw stage numbers and grouping
  by the retained subject's SRS system. Excluded content has a separate group with
  no system association. Review-only subjects acquire no invented assignment.
- Accuracy uses all retained review counters, including hidden and excluded
  content. Counters are widened before summing; percentages are computed only
  when the corresponding total is nonzero. Reported source percentages remain
  available as source state but are never averaged for status.

Loading and summarizing both validate the sync data; library callers who construct
or modify domain structs cannot silently obtain a summary of invalid data.
Status reads shared configuration to select its cache. It never writes or reads
lock contents. Cache errors direct users to a valid backup, another directory,
or a compatible Yomibu version; missing-cache guidance points to `yomibu sync` and the token environment
variable.

### Source boundary

Library callers may supply a trusted HTTPS base URL; plaintext HTTP is allowed
only for loopback IP addresses. Bases reject credentials, queries and fragments
and must end in `/`. The CLI has no endpoint override.

Fetch user, assignments, statistics, then sorted unique subject IDs in batches
of at most 100. Collections must end with explicit `pages.next_url: null`, even
after an empty page. Validate pagination before sending authorization.

Required nullable dates must be present, but may be null. Validate subjects before
applying content-access limits; invalid excluded records still fail. Returned
subjects must belong to the requested batch. Unexplained missing subjects fail.

See [the source adapter](crates/yomibu-components/src/wanikani_source/) and its
adjacent tests for normalization and transport behavior;
[the file adapter](crates/yomibu-components/src/file_learning_store/) and its
adjacent tests cover failures, interruption and locks.

## Reliability and storage

### HTTP boundary

The following defaults concern WaniKani synchronization. Story generation uses the explicit
one-attempt model-request limits, without this retry loop.

- Reuse one HTTP client. Default to a 10-second connection timeout and a
  30-second request timeout, including response-body retrieval.
- Request sequentially. Honor rate-limit reset information. Allow at most two
  retries per request for rate limiting and transient transport/server failures.
- Use 1- and 2-second transient-error backoffs. For rate limiting, respect the
  reset time; use 60 seconds when absent. Return an error rather than waiting
  more than 120 seconds for an individual reset.
- Do not retry authentication failures, malformed data, oversized pages or unsafe
  URLs. Transient and rate-limit failures share the two-retry budget.
- Retain a rate-limit deadline across cancellation; only a completed wait clears it.
- Disable redirects. Validate pagination URLs against the configured origin and
  exact collection path before attaching credentials; reject embedded credentials,
  fragments and repeated URLs.
- Bound response reads to 16 MiB per page. Reject oversized responses before
  deserialization, including bodies without a trustworthy Content-Length.
- These bounds apply per request/page, not to the total refresh duration or
  collection size. There is no whole-refresh deadline.
- Keep tokens out of serializable types, debug output, errors, fixtures, and
  logs. Report sanitized endpoint/status information rather than response bodies.
  Tests use synthetic credentials only.

### Cache lifecycle

Acquire the writer before fetching; reject an existing corrupt/unsupported cache
before network work. Validate replacement data and account identity before creating
a temporary file. Persist `wanikani.json` with a schema version. Reject unsupported versions
explicitly; introduce migrations only when needed. Preserve corrupt existing
caches for recovery rather than silently overwriting them.

Hold a separate advisory lock for the entire sync operation. A second writer
fails promptly; status can continue reading the previous sync data. Use
[standard-library file locking](https://doc.rust-lang.org/stable/std/fs/struct.File.html#method.try_lock),
available on current stable Rust. Keep the lock handle alive until sync ends;
do not delete and recreate a lock file while another process might hold it.
Dropping the guard explicitly unlocks before closing its file, so a descriptor
briefly inherited during concurrent process creation cannot extend the lock's
lifetime beyond the guard.

Write to a uniquely created temporary file in the cache directory, flush and
synchronize it, atomically replace the destination, then synchronize the parent
directory. New directories use 0700; new lock/cache files use 0600, subject to umask.
Do not broaden or rewrite existing directory permissions.
Failures before replacement preserve the previous cache. A directory-sync failure
after replacement must report uncertain durability rather than falsely claiming
the old cache survived.

Abrupt termination can leave private staging files. Loading and later writes
ignore them; there is no automatic cleanup. Existing readers retain a complete
old version after replacement. Process-kill tests do not establish power-loss durability.

## Code and repository structure

[The code map](ARCHITECTURE.md) locates source, tests, ownership boundaries and
build manifests. [AGENTS](AGENTS.md) holds engineering and documentation rules.
