# Yomibu

Yomibu is an unofficial WaniKani tool for personalized Japanese reading practice,
starting with a Rust CLI. Milestone 1 is complete through 1d: resilient full
synchronization, offline inspection of learner observations, and macOS/Linux CI.
The library composes sync/status through `App` with file or in-memory storage.
Offline story-request preview, explicit retrieval preparation, and a bounded
offline `analyze` command are available. The story path produces explicitly
requested experimental sentence candidates with local checks, manual/WaniKani
inventories, a topic brief, multiple targets and cached retrieval with offline
request preview; validated reading exercises remain deferred.

Offline preparation implementation and real-learner acceptance are complete as
of 2026-10-03; eligibility is not mastery and linguistic validity is unassessed.
A1's bounded analyzer and structural checks are reused by story assessment; its
research runner is retired. Engineering tests are separate from reference review
and held-out accuracy.
The investigation and private scoring are complete: held-out targets passed,
but three unsupported challenge Pass results make the frozen implementation a **no-go**
for the next stage. Model judgments remain provisional. See the
[implementation and evidence status](docs/A1_IMPLEMENTATION.md).
The separate [code follow-up](docs/A1_FOLLOWUP.md) now keeps object/predicate
combinations unresolved, including ordinary object sentences. It does not revise
the completed A1 score or establish a go decision.

```sh
WANIKANI_API_TOKEN=... cargo run -- sync
cargo run -- status
cargo run -- --data-dir /path/to/data status
```

Status reads `wanikani.json` in the selected directory (default `$HOME/.yomibu`).
It needs no token or network and never writes files. Errors return a nonzero exit
status with recovery guidance. The cache contains source learner state, not a
fixed definition of “known” material.

Sync reads the token only from the environment, fetches the complete account
state, and prints the saved summary. It holds an advisory writer lock and replaces
the cache only after validation. A different account requires another data
directory. Existing corrupt/unsupported caches are preserved for recovery.
Network, validation, and pre-replacement write failures leave the previous cache
usable. An error explicitly reporting uncertain durability means replacement
already occurred but synchronizing its directory failed.

To try the synthetic example without an account:

```sh
demo_dir=$(mktemp -d)
cp tests/fixtures/mixed.json "$demo_dir/wanikani.json"
cargo run -- status --data-dir "$demo_dir"
```

For synchronization and status, the recommended library entry point is `App`.
Within a caller-owned Tokio runtime with I/O and time enabled:

```rust,ignore
use yomibu::{App, adapters::{sources::wanikani::Client, stores::FileLearningStore}};

let mut app = App::new(FileLearningStore::new(data_dir))
    .with_source(Client::new(token)?);
let report = app.sync().await?;
let summary = app.status()?;
```

Replace the store with `InMemoryLearningStore::new()` for volatile storage; the
sync/status flow stays the same. `report.persistence` says whether the successful
write was volatile or durably acknowledged. Results own their data. Offline use
needs only `App::new(store).status()`, with no source or runtime. Store scope is
one WaniKani account, not an implicit current user. Constructors do not fetch or
write. Filesystem operations and domain calculations remain synchronous.

Lower-level access through `wanikani::Client`, `cache::SyncGuard`, `cache::load`,
and domain types remains available via adapter re-exports. Direct callers must
keep a sync guard alive around retrieval and replacement. Reuse the client to
retain rate-limit state. A custom base URL receives the supplied token and must
be trusted. Publicly constructed or directly deserialized sync data needs
validation; load, replace, and summarize validate automatically. Build API
documentation with `cargo doc --locked --no-deps` for typed error and cancellation
contracts; a persistence error does not always imply rollback.
Argument/environment handling, runtime startup, text output, and exit codes belong
to the binary. The repository pins Rust 1.98.1 with rustfmt and Clippy.

`domain::WaniKaniSyncData` replaces the earlier `domain::Snapshot` name;
`InvalidSnapshot` error variants are now `InvalidSyncData`. Rust callers must
update imports and matches. The schema-1 JSON key remains `snapshot`, so existing
cache files need no migration. This data covers one account's synchronization
interval, not the entire catalog or an instantaneous remote state.

## Learner eligibility and retired commands

`LearnerKnowledgePolicy::derive(&source)` explains which saved WaniKani materials
are eligible under the explicit `lesson-started` or `recorded-pass` rule. It keeps
source evidence and does not imply mastery or require grammar declarations.
Story inventory projection uses this same policy.

The old `preview` (first-N selection) and `prepare` (source-use inspection)
commands and examples are retired. Use `preview-story` for the exact offline
request and `prepare-retrieval` for embedding preparation. See
[command history](docs/COMMAND_HISTORY.md) for conclusions and a reproducible code pin.

## Analyze one supplied sentence offline

For normal use, explicitly import a verified bundle once, then use the managed
dictionary default. The existing developer installer supplies the publisher
bundle; only its explicit setup command may download:

```sh
python3 scripts/setup_a1_dictionary.py
cargo run --release --locked -- dictionary import --bundle target/a1/current
cargo run --release --locked --offline -- analyze --input tests/fixtures/analyze/nominal.json
cargo run --release --locked --offline -- dictionary verify
```

Managed storage defaults to `$HOME/.yomibu/dictionaries`; `--dictionary-dir PATH`
selects another private installation. Import copies and fully verifies the
dictionary and both notices. Normal startup checks installation records, sizes
and the dictionary header, then memory maps the checked file. Published files
must remain unchanged: reimport publishes a new generation and retains old ones.
Permissions and advisory locks cannot prevent arbitrary external writes.
See [dictionary installation and safety](docs/DICTIONARY.md).

Explicit `--dictionary PATH` retains full SHA-256 verification and owned bytes,
including paths inside managed storage. Use it for external files and recorded
reproducibility experiments. Existing developer examples remain:

```sh
# Explicit setup only if the pinned dictionary is not already installed:
python3 scripts/setup_a1_dictionary.py

# Completed Pass, Fail, and Inconclusive respectively (all exit zero):
cargo run --release --locked --offline -- analyze --dictionary target/a1/current/system_core.dic --input tests/fixtures/analyze/nominal.json
cargo run --release --locked --offline -- analyze --dictionary target/a1/current/system_core.dic --input tests/fixtures/analyze/unlisted.json
cargo run --release --locked --offline -- analyze --dictionary target/a1/current/system_core.dic --input tests/fixtures/analyze/object.json --json
```

`--offline` controls Cargo's dependency access; omit it for an initial build if
dependencies are not cached. The `analyze` command itself always operates offline.
Setup is a separate explicit download, never performed by analysis.

Use `--release` for interactive use: each invocation reads, verifies and
initializes the roughly 217 MB dictionary, which is much slower in a development
build. The first optimized compilation takes longer; unchanged builds are reused.

Supply your own UTF-8 JSON file with this small version-1 shape:

```json
{
  "version": 1,
  "sentence": "犬です。",
  "grammar": ["です — manual familiarity"],
  "bindings": {
    "vocabulary": [
      {"written_form": "犬", "reading": "イヌ", "sense": "dog", "direct_object": false}
    ],
    "grammar": [{"declaration_id": 1, "rule": "NominalDesu"}]
  }
}
```

All fields are required; unknown fields are errors. The document is limited to
64 KiB (65,536 bytes) and the nonblank sentence to 100 Unicode scalar values.
Text is retained without trimming or normalization. Empty vocabulary, grammar
declarations or grammar bindings are valid and grant no corresponding permissions.
Declarations keep their order and duplicates; `declaration_id` is their one-based
position. Free-form descriptions never enable grammar automatically. Ordinary
inputs require no research-packet reuse metadata, case IDs, or reference labels.

Use exact dictionary-style **katakana** readings and nonblank sense labels in
whole vocabulary tuples. These are explicit user declarations, not verified facts.
`direct_object: true` asserts transitive-use evidence for that tuple; it cannot
establish that an object/predicate combination is compositional. False supplies
no such evidence. The existing explicit grammar rule names are:

| Rule | Bounded use |
| --- | --- |
| `NominalDesu` | Nominal です |
| `TopicWa` | Scoped nominal topic は |
| `ObjectWo` | Scoped direct object を; combination remains unresolved |
| `PoliteNonPast` | Regular godan/ichidan ます |
| `PolitePast` | Regular godan/ichidan ました |
| `PoliteNegativeNonPast` | Regular godan/ichidan ません |
| `PoliteNegativePast` | Regular godan/ichidan ませんでした |

Text output names all five checks, their coverage and reasons. Findings identify
the original affected text without color, for example
`bytes 0..3: "犬" — whole word is not permitted`. Ranges are half-open UTF-8 **byte**
offsets in the original sentence, not character positions in the escaped display.
Untrusted text is escaped. `--json` returns `version`, the exact decoded `input`,
full `analysis` (C/A tokens, spans and pinned provenance), `outcome`, and
`evaluation` (all checks, reasons, spans and assessment limitations).
Completed outcomes use `{"Completed":"Pass"}`, `{"Completed":"Fail"}`, or
`{"Completed":"Inconclusive"}` and all exit successfully. Input/execution errors
exit nonzero, print an escaped diagnostic to stderr and publish no evaluation on
stdout, including with `--json`.
Argument errors retain readable usage/help lines while escaping supplied values;
usage and help always name the executable `yomibu`.

An explicit external dictionary path needs no HOME. With that selection, analysis
reads only the supplied input and dictionary files; it needs no HOME,
credentials or learner state, ignores `--data-dir`, and performs no writes, sync,
network calls, automatic downloads or telemetry. Naturalness, multiword expressions,
and contextual reading/sense remain unassessed. Object sentences remain Inconclusive
even with transitive word evidence, unless an established permission failure makes
the outcome Fail; its reason/span remains visible beside the uncertainty.
A Pass is never an accepted exercise.

This is a separate CLI milestone exposing unchanged library judgments. The
[synthetic contract fixtures](tests/fixtures/analyze/README.md) and tests are
engineering evidence, not a new linguistic evaluation or a repaired A1 score.
Direct Rust callers continue to compose `SudachiAnalyzer::load`,
`Sentence::new`, `analyzer.analyze`, and `evaluation::evaluate` with explicit
`GrammarDeclarations` and `EvaluationBindings`; no CLI or store is required.

## Shared experimental story candidates

Use one inventory from manual JSON, an explicit WaniKani cache, or both. A separate
story request gives the topic/scene and lists vocabulary and grammar targets.
Selection chooses supporting words; assessment keeps the complete inventory.

```sh
cargo run --locked -- prepare-retrieval \
  --inventory tests/fixtures/story/inventory.json \
  --request tests/fixtures/story/request.json \
  --embedding-cache /tmp/yomibu-vectors.json --embedding-provider lexical-baseline
cargo run --locked -- preview-story \
  --inventory tests/fixtures/story/inventory.json \
  --request tests/fixtures/story/request.json \
  --embedding-cache /tmp/yomibu-vectors.json --json
```

Preview is offline and initializes no dictionary, credential, client or runtime.
The lexical baseline above is explicitly nonsemantic. Local and hosted embedding
adapters are available, but no dense model is selected by default; see the
[retrieval comparison and limitations](docs/RETRIEVAL.md).

To generate, use `generate-story` with the same input/cache flags plus
`--allow-model-call` and `--dictionary PATH` (or `--dictionary-dir PATH`). This
requires `OPENAI_API_KEY` and makes one paid generation attempt. Set
`--candidates N` (default 2) on generation or preview; this is an execution option,
separate from the story request. All original texts and partial assessments are reported; no automatic
retry or accepted exercise is produced. Hosted embedding preparation has its own
separate opt-in. See [story usage, library flow and limits](docs/STORY_GENERATION.md).

The former `generate-candidates`, `generate-focused`, `context-preview` commands
and `--permissions` flag have no aliases. Historical G1/G2 evidence and reproducible
code pins are preserved in [generation history](docs/GENERATION_HISTORY.md).

## Bounded offline analysis investigation (A1; historical no-go)

```sh
# Explicit one-time download of the pinned public dictionary, outside Git:
python3 scripts/setup_a1_dictionary.py
```

Setup verifies the dictionary and both notices as one bundle, then publishes it
through an atomic `current` link. Rerun setup for the new layout; older flat files
stay untouched. Existing complete bundles survive failures before publication;
errors after publication distinguish uncertain durability.

The research packet runner is retired. Its three-case smoke input, protocols,
reports and conclusions remain historical evidence; reproduce the runner in the
pinned checkout documented in [A1 evidence](docs/A1_IMPLEMENTATION.md).
Current story, evaluation and real-analyzer tests protect Pass/Fail/Inconclusive,
execution errors/NotRun, partial results and original UTF-8 spans. The current
`analyze` CLI and library functions remain available for supplied text. These
checks never certify accepted exercises; naturalness, idioms and contextual
reading/sense remain unassessed.

The [active visible draft, revision 2](tests/fixtures/a1/review-draft-v2.json)
now has **frozen provisional references and a completed visible evaluation**. Review of the
[preserved original](tests/fixtures/a1/review-draft.json) and one focused
clarification left two proposed positives unresolved. Both affected pairs were
replaced before freezing, with the full history retained. Revision 2 keeps
24 supported development outcomes and all 12 challenge references after the
four replacement reviews were reconciled. The
[holdout custodian prompt](docs/A1_HOLDOUT_CUSTODIAN_PROMPT.md)
keeps the 24 held-out core references in a separate context. On 2026-10-04 the
custodian reports all 24 reviewed, reconciled and frozen, after replacing ten
exposed cases with history retained. The visible run matches all 24 development
outcomes and negative reason/span checks, but three unsupported challenge Pass
results prevent a bounded go. The released held-out input matched its freeze hash
and ran once offline using the frozen executable: 24 cases, 120 completed checks,
zero execution errors. The private scoring receipt reports all held-out targets
met: 24/24 outcomes, 120/120 check judgments, and 11/12 exact negative reason/span
matches. One span discrepancy remains recorded. A1 is complete with a no-go;
held-out success does not override the failed challenge safeguard. See the
[evaluation status](docs/A1_EVALUATION_STATUS.md). No accepted exercises or
independently validated linguistic ground truth are claimed.

Current code keeps Particles and Scope Inconclusive for object/predicate
combinations whose multiword use it cannot assess, even when the verb has an
explicit transitive-use binding. Existing permission failures still produce Fail.
This addresses the three exposed Pass results by reducing supported coverage;
it also withholds Pass from two ordinary visible development positives. The
[follow-up record](docs/A1_FOLLOWUP.md) separates these development results from
the preserved historical evaluation.

The [design vocabulary](SPEC.md#design-vocabulary-and-composition) distinguishes
implemented knowledge/preparation types from future components such as
`ExerciseGenerator`. Validated generation, multi-source learners, SQL, and Cloud remain
future work; [ARCHITECTURE.md](ARCHITECTURE.md) records their intended composition.

The repository is a Cargo workspace. `crates/yomibu` contains the reusable
library; `crates/yomibu-cli` contains the executable named `yomibu`. Start at
[`plan_generation` and `generate_story`](crates/yomibu/src/story/mod.rs) for the shared
story sequence. Their stage calls point to sibling `request.rs`, `selection.rs`,
`model_request.rs` and `assessment.rs` files. Evaluation groups result types,
supported constructions and full-inventory checks under `evaluation/`; smaller
modules stay flat. Public library imports remain unchanged.
The CLI calls these functions and owns files, credentials,
runtime, terminal rendering and exit status; other callers reuse the same sequence
and structured `reports` projections. CLI `main.rs` handles parsing and exit status,
`args.rs` defines flags, `commands/` configures concrete resources, and `output/`
formats terminal output. Reusable cache preparation and explicit-path file adapters
are in the library.
Root CLI commands above still work. Use `cargo test --locked -p yomibu`
for library tests or `cargo test --locked -p yomibu-cli` for CLI tests; the checks
below cover both packages. Shared fixtures remain under `tests/fixtures` and the
pinned dictionary under `target/a1/current`. An API crate will be added when its
HTTP application is implemented.

Real A1 adapter tests require the pinned dictionary. Run the setup command above
first (Python 3.8+); tests fail clearly if it is unavailable and never download or
substitute it automatically. Subsequent analysis and tests use the local file.

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
git diff --check
```

The [CI workflow](.github/workflows/ci.yml) runs those gates on `ubuntu-latest` and
`macos-latest` in parallel for pushes to `main` and pull requests. New commits
cancel older runs for the same branch or PR. It installs the repository toolchain,
requires the committed lockfile, and checks that it stays unchanged. It explicitly prepares the pinned A1 dictionary. Both CLI
and direct-library demonstrations run with synthetic input; CI also checks diff
whitespace and builds API documentation with warnings denied. Rust
dependency downloads and compiled dependencies are cached separately by platform
and compiler; only `main` saves caches, and PRs can restore them. Cache misses
still run every gate. No WaniKani secret is required.

The test profile optimizes only `sha2` (`opt-level = 3`) to reduce repeated full
dictionary hashing, including CLI binaries built by `cargo test` and child test
processes. Verification still precedes dictionary use, with the verified bytes
owned by the analyzer. Development/release profiles, dependency features and
all coverage are unchanged. Explicit refactor review found no source/test change
justified. On Linux/x86_64, two prebuilt `analyze_cli` runs fell from
72.80/72.20s to 5.96/5.90s; full-suite execution fell from 202.31s to 28.93s.
Fresh prebuilds took 81.07s/81.51s. Hosted macOS test execution fell from
266.03s to 55.69s, while test compilation rose from 16.87s to 27.80s.
These are observations, not timing gates or a macOS guarantee;
[PLAN.md](PLAN.md#sha-256-test-profile-optimization--2026-10-05)
records the method and hosted platform evidence. To separate compilation from
execution, prebuild with `cargo test --locked --all --no-run`, then run
`cargo test --locked --test analyze_cli` twice and `cargo test --locked --all`.

See [SPEC.md](SPEC.md) for authoritative requirements,
[ARCHITECTURE.md](ARCHITECTURE.md) for responsibilities, file/repository structure,
and composition examples, [PLAN.md](PLAN.md) for milestones and TDD evidence, and
[AGENTS.md](AGENTS.md) for engineering rules.

## Rust notes for a Ruby developer

- `LexicalContent` is an enum with three data shapes. Pattern matching handles
  those shapes explicitly; kana-only vocabulary cannot hold a readings field.
- `Option` distinguishes absence from a value: missing lifecycle dates, unknown
  SRS systems, and no-review accuracy are different from zero-valued data.
- Summaries own one username string and their aggregates, so they outlive the
  input and application. Source collections are inspected by reference.
- `App<Store, Source>` selects dependencies through Rust generics. The compiler
  checks their contracts; no runtime dependency-injection container is needed.
  An `Arc` shares an immutable data version without copying its collections.
- `CacheError` and `ValidationError` are typed library boundaries. `?` propagates
  failures; `anyhow` is confined to executable orchestration and exit handling.
- Ordered maps make SRS output deterministic. Unlike Ruby's growable integers,
  Rust integers have fixed widths: review counts widen from `u64` to `u128`
  before aggregation, with conversion to floating point only for percentages.
- `LearnerKnowledge<'a>` borrows source evidence. Rust checks that it remains alive
  while the view is used. Changing policy recomputes eligibility without rewriting
  learner data.
- `Sentence<'a>` borrows unchanged input; A1's enums distinguish a completed
  judgment from uncertainty, an execution error, or a check not run. The concrete
  analyzer owns verified dictionary bytes. Boxing a large upstream error preserves
  its source without enlarging every successful Result.
- Async is confined to HTTP and retry waits. Validation, summary calculation,
  locking, and cache persistence stay synchronous.
- `SyncGuard` owns a file handle: leaving scope releases the lock even when `?`
  returns early. This replaces manual unlock/ensure bookkeeping.
- Private transport structs move strings and vectors into domain types. Serde's
  `Serialize` derives write the owned schema without storing API envelopes.
- `PartialEq` compares retained resource data for duplicate reconciliation; source
  timestamps participate, while ignored API preferences and mnemonics do not.
- `WriteError::BeforeReplacement` and `DurabilityUncertain` model different
  outcomes. A single generic exception would make safe recovery harder.
  `#[source]` exposes the underlying I/O cause through the standard error chain;
  including it in an error's display text alone does not do that.
- Dropping an async future cancels its work at an `await`. A saved rate-limit
  deadline stays in the client until the wait finishes, so a later fetch still
  observes the reset time.
- `tokio::join!` overlaps independent HTTP test scenarios on one runtime. Each
  keeps its own server/cache and real retry waits; a separate virtual-clock test
  checks the exact backoff deadlines without socket I/O.

The test suite (including two subprocess helpers) uses local mock/raw HTTP
servers, isolated directories, and child processes. It covers streamed limits,
deadlines, retry budgets, hostile pagination, later-page failures, storage faults,
writer contention, and process termination. Killed writers can leave private
staging files that later reads/writes ignore. Fault injection and process-kill
tests do not simulate power loss. The pre-migration 58-entry suite, formatting,
and Clippy passed on native macOS/arm64 and Debian Linux/arm64 in a container,
then on GitHub-hosted Ubuntu/x86_64 and macOS/arm64. Current migration verification
is recorded separately in `PLAN.md`; earlier runs do not validate later code.
No live account was used. Request deadlines and page limits
do not bound total refresh duration or collection size. See the 1d record in
[PLAN.md](PLAN.md) for full results and limits.

A test scheduling follow-up retained every HTTP/cache scenario and strengthened
the timer assertions. Native macOS test execution fell from about 17.8s to 8.1s,
excluding compilation. See the follow-up in [PLAN.md](PLAN.md) for validation and
hosted CI measurements; runner setup and compilation still contribute to CI time.
