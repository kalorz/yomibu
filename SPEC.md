# Yomibu specification

This is the authoritative product and architecture baseline, accepted on
2026-09-27. Changes to these decisions must be reflected here and in `PLAN.md`.
Future capabilities described below are direction, not authorization to implement
them in milestone 1.

## Product

Yomibu generates personalized Japanese reading practice. Building a useful
application and learning idiomatic production Rust are equally important. Prefer
the simplest idiomatic design that solves a current problem; do not translate
Ruby object and service conventions mechanically or add abstractions to teach
language features.

The first interface is a CLI. Domain behavior and integrations must also be
callable through a library without starting a CLI process. Later interfaces may
include web applications, native applications, and an MCP server.

### Practice direction

1. Combine WaniKani learner state with Yomibu-owned grammar knowledge, initially
   entered through a local file.
2. Accept explicit practice targets; later select targets using WaniKani
   difficulty and Yomibu mistakes.
3. Retrieve structured lexical information and examples for the targets.
4. Generate two short, coherent candidate passages in one LLM request.
5. Validate candidates deterministically, select the best valid candidate, and
   use an LLM repair loop only when none pass.
6. Add broader grounded criticism for naturalness and grammar suitability, plus
   reading/translation quizzes. Focused checks of intended sense and contextual
   reading are required as soon as ambiguous targets are supported.
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

Do not define a permanent meaning of "known" in milestone 1. Preserve the source
learning state so policy can evolve independently. Later validation must address
inflections, particles, lexical ambiguity, and words absent from WaniKani.
Membership checks alone cannot establish complete linguistic correctness.

### Intended readings and senses

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
| WaniKani | Primary learner knowledge, lexical information, review statistics, and context sentences |
| Yomibu history | Attempts and mistakes made in Yomibu |
| Yomibu grammar knowledge | Learner data, initially entered through a local file |
| JMdict | Optional enrichment for useful reading/sense distinctions, not a mandatory lookup |
| Trusted external sentence corpora | Optional later grounding when their value is demonstrated |

Grammar knowledge is evolving learner data, distinct from application preferences
such as desired passage length. Its initial file is an input and persistence
mechanism, not a permanent domain boundary. When a database is introduced, store
grammar knowledge alongside vocabulary progress and attempt history; files may
remain optional import/export rather than a competing authoritative copy.
Bunpro may later supply grammar state through mapped grammar identifiers and
source information. Do not implement that integration or its schema now.

Retrieval starts with structured lookups. RAG does not require vector search.
Embeddings become appropriate only for a demonstrated retrieval problem, such as
ranking a large sentence corpus or selecting semantically useful known words.

Milestone 1 contains no LLM integration, generation, quiz, RAG implementation,
embeddings, vector database, PostgreSQL, Bunpro, JMdict, or web interface, including
placeholder versions. Local files are sufficient for the CLI proof of concept.
PostgreSQL is the likely later web database; pgvector remains conditional on a
real vector-search need.

## Milestone 1: WaniKani synchronization

Supported platforms are macOS and Linux. Use one account per data directory and
full refresh synchronization.

### CLI and local files

Milestones 1a–1d implement `status`, the complete `sync` path, resilience
verification of the HTTP adapter, cache writer, and advisory lock, macOS/Linux
CI, and the public-surface review. Actual validation and limitations are recorded
in `PLAN.md`.

The complete milestone 1 command set is:

```sh
WANIKANI_API_TOKEN=... yomibu sync
yomibu status
```

Both accept a global `--data-dir PATH`. The default is `$HOME/.yomibu`; if HOME is
unavailable or empty, require an explicit directory. Read `WANIKANI_API_TOKEN`
only for `sync`. Do not accept or persist a token through a CLI argument or config file.

```text
~/.yomibu/
  wanikani.json
  wanikani.lock
```

`wanikani.lock` is a separate advisory lock file; its existence alone does not
indicate a running sync. Reserve `config.toml` for later application settings.
Milestone 1 neither creates nor parses it and does not persist grammar knowledge.

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
- Store synchronization start and completion times. The snapshot contains
  observations collected during that interval; it is not a transactionally
  consistent snapshot of the remote system.
- A complete refresh replaces prior records rather than merging them, so removed
  or reset state does not survive through stale local records.
- A different account must not replace an existing account's cache. Return an
  actionable error directing the user to another data directory.
- Do not request individual review history, study materials, or the remote summary
  endpoint in this milestone. Status is derived locally.

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

### Schema 1 and status count semantics (implemented in 1a)

The cache envelope is `{ "schema_version": 1, "snapshot": { ... } }`.
`Snapshot` contains the synchronization interval, learner, subjects, assignments,
review statistics, and `unavailable_subjects`. Subject lexical content is tagged
as `kanji`, `vocabulary`, or `kana_vocabulary`; only the first two have readings.
The cache contains normalized Yomibu data, not WaniKani response envelopes. See
`src/domain.rs` for the concrete field types and `tests/fixtures` for synthetic
examples. Nullable dates use `Option<DateTime<Utc>>`; missing statistics remain
absent records. Additional JSON fields are tolerated.

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

Loading and summarizing both validate the snapshot; library callers who construct
or modify domain structs cannot silently obtain a summary of invalid data.
Status never creates directories or files or reads config/lock contents. Cache
errors direct users to a valid backup, another directory, or a compatible Yomibu
version; missing-cache guidance points to `yomibu sync` and the token environment
variable.

### First complete sync implementation (1b)

`wanikani::Client::new` uses the official HTTPS origin. Library callers may use
`Client::with_base_url` with a trusted HTTPS base URL; plaintext HTTP is accepted
only for loopback IP addresses, enabling local contract and cross-component tests.
Base URLs reject embedded credentials, queries, and fragments and must end in `/`.
The CLI exposes no endpoint override. DTOs remain private in `wanikani::dto`.

Retrieval is sequential: user, assignments, review statistics, then sorted unique
subject IDs in batches of at most 100. Collections must explicitly terminate with
`pages.next_url: null`, even after an empty page. Pagination checks origin, exact
collection path, embedded credentials, fragments, and repeated URLs before sending
authorization. Declared and streamed page sizes are bounded; redirects are disabled.
The reusable client retains a rate-limit deadline between requests and refreshes.
Transport failures and server errors use bounded retries; authentication, malformed
data, oversized pages, and invalid URLs fail without retrying.

Source nullable dates must be present but may be null. Source subjects are validated
before applying the learner's content-access limit, so an invalid excluded record
cannot disappear behind an exclusion marker. Returned subjects must belong to the
requested batch; unexplained missing subjects still fail validation. No remote
summary, reviews, study materials, or per-subject lookup is requested.

The CLI acquires `cache::SyncGuard` before retrieval and keeps it through persistence.
Acquisition rejects an existing corrupt/unsupported cache before network requests.
`SyncGuard::replace` validates the proposed snapshot and checks the current cache's
account identity before creating a private temporary file. It writes schema 1,
flushes and synchronizes the file, persists it atomically, then synchronizes the
parent directory. New directories use mode 0700; new lock/cache files use 0600
(subject to umask). Existing directory permissions are not broadened or rewritten.
`WriteError::DurabilityUncertain` distinguishes post-replacement directory-sync
failure from errors that leave the destination unreplaced.

Successful sync prints the same cached-observation summary as offline status.
Tests cover mock API → normalized snapshot → private cache → credential-free
`status` subprocess, repeated refresh/removal, account mismatch, basic lock
contention, and the CLI composition's lock lifetime. Milestone 1c expands these
checks as described below. The 1d acceptance record covers Linux and CI. No live
account verification is claimed.

### Sync resilience (1c)

A client retains its rate-limit deadline if a caller cancels a fetch during the
wait. Only completion of that wait clears the deadline. Reusing the client
therefore continues to respect the server's reset time. The 10-second connection
and 30-second request defaults, two-retry budget, 16 MiB page bound, cache schema,
public API, and CLI options are unchanged.

Local tests exercise stalled headers and bodies, truncated Content-Length and
chunked bodies, successful transport recovery, exact-limit and oversized streamed
pages, and rejection of declared oversized bodies before reading them. Rate-limit
waits use a manually advanced clock; socket I/O runs with real time. Permanent
HTTP errors, redirects, credential-bearing pagination, and unsafe/repeated URLs
fail without leaking credentials. Mixed transient failure categories share the
same retry budget.

Later-page failures on assignments, statistics, and subjects preserve the old
cache byte for byte and leave offline status usable. Tests also cover invalid
excluded content, unexpected/missing subjects, conflicting source data, changing
access limits, and removal of review-only state without fabricated records.

Private storage checkpoints allow deterministic errors and process interruption
before temporary-file creation, encoding, flushing, file sync, replacement, and
directory sync. These are not library configuration or CLI options. Tests verify
temporary-file cleanup after handled failures, a real rename failure, and a
synthetic directory-sync failure mapped through the same error boundary as an
actual directory error. The latter leaves the complete new cache visible and
returns `DurabilityUncertain`; no rollback is claimed.

Separate processes verify lock contention, offline readers during retrieval,
termination during retrieval and at each write boundary, lock release on process
death, and recovery using the same lock file. An already-open reader retains its
complete old file after replacement. Abrupt termination can leave a private
staging file; it is ignored by loading and later writes, and is not automatically
deleted. Process-kill tests do not simulate power loss or establish filesystem
durability across a machine crash.

### Milestone acceptance (1d)

GitHub Actions runs formatting, Clippy with warnings denied, and all tests on
macOS and Linux for pushes and pull requests. The workflow uses the repository
toolchain file, requires a committed `Cargo.lock`, passes `--locked` to Clippy and
tests, and verifies the lockfile remains unchanged. Tests use local HTTP servers
and synthetic credentials; CI requires no live WaniKani account.

Public API documentation records the caller's responsibilities: hold `SyncGuard`
across fetch and replacement, enable Tokio I/O/time for retrieval, trust any
custom API origin that receives a token, and validate publicly constructed data
before direct use. Loading, summarization, and replacement validate automatically.
HTTP errors remain sanitized; `WriteError::DurabilityUncertain` exposes its
underlying I/O cause without changing the already-replaced outcome.

Quality gates passed on native macOS/arm64 and containerized Debian Linux/arm64.
GitHub-hosted runs are unverified until the workflow is pushed; see `PLAN.md` for
the exact checks, TDD evidence, and remaining verification limits.

## Rust architecture

Use one package with a library target and thin binary. No multi-crate workspace
is needed.

- The binary owns argument parsing, environment access, runtime startup,
  presentation, and exit status.
- The HTTP adapter asynchronously produces a normalized snapshot.
- Domain validation and summary calculation are synchronous.
- Cache loading, locking, and persistence are synchronous library operations.
  The CLI performs them outside its async fetch operation.
- Expose only the client, snapshot/domain types, cache operations, summary result,
  and meaningful typed errors. Keep WaniKani transport DTOs private.
- Use concrete types and functions initially. No repository traits, service-object
  hierarchy, or dependency-injection framework is needed.
- Use ownership and borrowing deliberately: move data when building the snapshot
  and borrow it for read-only work. Avoid cloning solely to bypass ownership
  design problems.
- Use `Option` for real absence and `Result` for recoverable errors. Do not use
  production `unwrap()` or `expect()` for recoverable situations. No unsafe code
  is needed for milestone 1.

The stable toolchain reverified and installed on 2026-09-27 is Rust 1.98.1; the
current stable edition is 2024. Initialization pinned that exact version in `rust-toolchain.toml`,
with rustfmt and Clippy. Keep `Cargo.lock` committed. Upgrade the pin deliberately
with validation rather than relying on each developer's global default.

Sources: [Rust release](https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/),
[edition guide](https://doc.rust-lang.org/edition-guide/rust-2024/index.html),
[toolchain configuration](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file).

### Dependencies

| Dependency | Justification |
| --- | --- |
| `clap` | CLI arguments and help |
| `reqwest`, `tokio` | HTTP requests, deadlines, and bounded retry waits |
| `serde`, `serde_json` | API decoding and versioned JSON persistence |
| `chrono` | Parsed UTC timestamps and timestamp arithmetic |
| `thiserror` | Typed library errors |
| `anyhow` | Contextual errors at the executable boundary |
| `tempfile` | Safely created temporary files for atomic replacement |
| `wiremock` (development) | Local HTTP contract tests |

Milestone 1b adds reqwest and Tokio at runtime, promotes tempfile to a runtime
dependency for safe persistence, and adds wiremock for development. Tokio uses a
current-thread runtime in the binary; only HTTP and retry waits are asynchronous.
Milestone 1c adds only Tokio development features for raw local HTTP servers,
process coordination, and controlled time; no new crates or runtime features.
Proptest remains deferred because the current behavior is covered by concrete
contract and integration tests. Exact resolved versions remain in `Cargo.lock`.
The 1d feature-graph review retained the existing dependency declarations and
lockfile without changes.

Enable only required features. Use reqwest's Rustls support. Defer tracing until
diagnostic needs justify it. Use standard-library facilities for CLI subprocess
tests unless an additional testing dependency demonstrates value.

Documentation verified on 2026-09-27 lists reqwest 0.13.5, Tokio 1.53.1, and clap
4.6.7. These are dated observations, not permanent version requirements. Resolve
stable compatible versions during initialization and record exact resolutions in
the lockfile. See [reqwest](https://docs.rs/reqwest/latest/reqwest/),
[Tokio](https://docs.rs/tokio/latest/tokio/), and
[clap](https://docs.rs/clap/latest/clap/).

## Reliability and storage

### HTTP boundary

- Reuse one HTTP client. Default to a 10-second connection timeout and a
  30-second request timeout, including response-body retrieval.
- Request sequentially. Honor rate-limit reset information. Allow at most two
  retries per request for rate limiting and transient transport/server failures.
- Use 1- and 2-second transient-error backoffs. For rate limiting, respect the
  reset time; use 60 seconds when absent. Return an error rather than waiting
  more than 120 seconds for an individual reset.
- Do not retry authentication failures or malformed data.
- Disable redirects. Validate pagination URLs against the configured origin and
  collection path before attaching credentials; detect repeated pagination URLs.
- Bound response reads to 16 MiB per page. Reject oversized responses before
  deserialization, including bodies without a trustworthy Content-Length.
- These bounds apply per request/page, not to the total refresh duration or
  collection size. There is no whole-refresh deadline in milestone 1.
- Keep tokens out of serializable types, debug output, errors, fixtures, and
  logs. Report sanitized endpoint/status information rather than response bodies.
  Tests use synthetic credentials only.

### Cache lifecycle

Persist `wanikani.json` with a schema version. Reject unsupported versions
explicitly; introduce migrations only when needed. Preserve corrupt existing
caches for recovery rather than silently overwriting them.

Hold a separate advisory lock for the entire sync operation. A second writer
fails promptly; status can continue reading the previous snapshot. Use
[standard-library file locking](https://doc.rust-lang.org/stable/std/fs/struct.File.html#method.try_lock),
available on current stable Rust. Keep the lock handle alive until sync ends;
do not delete and recreate a lock file while another process might hold it.
Dropping the guard explicitly unlocks before closing its file, so a descriptor
briefly inherited during concurrent process creation cannot extend the lock's
lifetime beyond the guard.

Write to a uniquely created temporary file in the cache directory, flush and
synchronize it, atomically replace the destination, then synchronize the parent
directory. Create private directory/file permissions on supported platforms.
Failures before replacement preserve the previous cache. A directory-sync failure
after replacement must report uncertain durability rather than falsely claiming
the old cache survived.

Atomic replacement alone does not synchronize file contents or the parent
directory; both steps are explicit. See
[tempfile persistence behavior](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist).

## Milestone 1 structure

The tree below shows the implemented milestone 1 structure, including the CI
workflow added in 1d. Later milestones require separate authorization.

```text
yomibu/
├── SPEC.md
├── PLAN.md
├── AGENTS.md
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── .gitignore
├── .github/workflows/ci.yml
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── domain.rs
│   ├── cache.rs
│   ├── cache/
│   │   └── tests.rs
│   ├── summary.rs
│   └── wanikani/
│       ├── mod.rs
│       ├── dto.rs
│       ├── resilience.rs
│       └── tests.rs
└── tests/
    ├── cache.rs
    ├── cli.rs
    ├── summary.rs
    ├── sync.rs
    └── fixtures/
        └── wanikani/
```

HTTP adapter tests may live beside private adapter code, permitting mock-server
configuration without exposing test-only CLI options. Module placement does not
change their role as tests across the HTTP boundary.

## Rust notes for a Ruby developer

- **Enums model distinct shapes.** Kanji, vocabulary, and kana-only vocabulary
  have different valid fields. Variants express that directly instead of using
  objects with many conditionally meaningful attributes.
- **`Option` preserves absence.** No review-statistic record is different from a
  record containing zero counters. Pattern matching keeps that distinction
  explicit.
- **Ownership clarifies data flow.** Move decoded strings and collections into
  the snapshot; borrow it for summaries and serialization. A shared mutable
  object graph is unnecessary.
- **Typed errors support callers.** Library users can distinguish authentication,
  invalid data, and storage failures. `?` propagates errors without flattening
  them into undifferentiated strings.
- **Traits are not mandatory interfaces.** Concrete HTTP and file adapters already
  provide usable boundaries. Local servers exercise the actual adapter without
  mocking domain behavior.
- **RAII protects resources.** A lock guard owns its file handle. Leaving scope
  releases the handle and lock, including during error propagation.
- **Domain data need not encode storage.** Grammar knowledge can be an ordinary
  struct passed into generation, with file or database loading outside that
  logic. This separation does not require a repository trait in advance.
