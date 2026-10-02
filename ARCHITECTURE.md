# Yomibu architecture

Accepted direction as of 2026-10-02. `SPEC.md` defines product requirements and
invariants; this document defines responsibilities, composition, and code
boundaries; `PLAN.md` records delivery and verification. Future examples here
describe intended contracts, not implemented features or authorization to build
every adapter.

## Delivery boundary

The completed migration covers the existing WaniKani `sync` and offline `status`
use cases, an application entry point, and interchangeable file and in-memory
stores. Offline preparation adds explicit grammar-file input, on-demand knowledge
policy, and cached lexical retrieval. Generation, grammar database persistence,
multi-source learner management, PostgreSQL, Cloud HTTP endpoints, and additional
provider integrations follow later.
Keep one Cargo package with a library and a thin CLI binary.

The completed **G0 — Manual candidate preview** slice in `PLAN.md` adds
explicit in-memory word/grammar input, deterministic selection, independent
result checks, and a CLI entry point. Its synchronous library operation has no
store, account, runtime, or service dependency. The existing storage/source
contracts are unchanged; the future generation pipeline remains deferred.

Do not replace the existing safety behavior during migration: source-state
preservation, full-refresh replacement, account protection, writer exclusion,
schema-1 compatibility, credential isolation, and distinct pre-replacement and
uncertain-durability errors remain required.

## Domain and vocabulary

Use the responsibility definitions in [SPEC.md](SPEC.md#design-vocabulary-and-composition).
The central distinctions are:

- **Language material:** what a character, word, or grammar entry means, including
  readings, descriptions, examples, identifiers, and provenance.
- **Learner progress:** source observations and manual declarations about a
  learner. Retain source state rather than a permanent `known` flag.
- **Learner knowledge:** the result of applying `LearnerKnowledgePolicy` to the
  selected progress. It is derived, explainable, and recomputable.
- **Source connection:** one learner's specific provider account or input. A
  provider kind is not a connection ID, and a provider account ID is not the
  future Yomibu learner ID.
- **Prompt template:** stored reusable instructions. **Model request:** prepared
  input for one invocation, including the selected exercise data and options.

Grammar entries keep provider-scoped IDs. Bunpro and Renshuu entries are not
automatically declared equivalent. Manual declarations receive technical IDs;
the learner supplies a description, not an ontology identifier. Never infer a
global count of unique grammar rules from unrelated provider records.

Application settings, learner data/source selections, and exercise requests are
separate inputs. Do not reintroduce an all-purpose `ProfileConfiguration`.

## Responsibilities and dependency direction

```mermaid
flowchart TD
  CLI[CLI composition] --> App
  HTTP[Future Cloud composition] --> App
  App --> Progress[LearnerProgressStore]
  App --> Policy[LearnerKnowledgePolicy]
  App --> Sources[Source connections and adapters]
  App --> Writer[SourceSyncWriter]
  App --> Generator[ExerciseGenerator]
  Generator --> Materials[LanguageMaterialStore]
  Generator --> Examples[ExampleSearch]
  Generator --> Candidates[CandidateGenerator]
  Prompted[PromptedCandidateGenerator] -. implements .-> Candidates
  Prompted --> Prompts[PromptStore]
  Prompted --> Model[LanguageModel]
```

This diagram is the target responsibility model. The current sync/status slice
uses a physical `LearningStore` with a consistent read and an exclusive
`SourceSyncWriter`; it does not pretend that a WaniKani batch is generic learner
knowledge. Its `LearningSource` supplies normalized `WaniKaniSyncData` for this
slice. Generalize that source contract when a second real integration requires
it; do not erase provider semantics preemptively.

`FileLearningStore` and `InMemoryLearningStore` are physical backends. Their
current coherent read serves offline source summaries and preparation. Later
material/progress read ports can share the same backend and transaction boundary. This is not a
third repository of snapshots: `WaniKaniSyncData` is the transfer value containing
related material and progress from one synchronization interval.

Adapters implement small capabilities owned by the library. `App` depends on
those contracts, not HTTP or filesystem details. Domain validation is synchronous
and independent of adapters. No dependency-injection container, service locator,
per-field service objects, or mutable current-user singleton is needed.

Public extension points are justified by actual substitution needs. Use concrete
values/functions for deterministic logic. Code may be public without making all
its Rust implementation details `pub`. A trait does not require dynamic dispatch;
choose generics or trait objects according to actual composition needs.

## Execution and ownership

The executable owns argument parsing, settings, secret resolution, runtime
startup, output, and exit status. Library code does not read process environment,
start a runtime, or print. Constructors compose/configure; explicit methods open
resources and execute work.

For the current CLI:

1. Parse the command. Resolve a data directory for sync/status/prepare; only `sync`
   resolves the WaniKani token. Preview parses structured entries and directly
   calls the synchronous library operation described below.
2. For sync/status, construct `FileLearningStore` for that directory and `App`.
   For `sync`, supply the WaniKani client as the source. `status` needs no source
   or HTTP client.
3. `App.sync` acquires a store writer before network retrieval. A corrupt cache
   or conflicting writer fails before fetching. The writer lives across fetch
   and replacement and is released on success, error, or future cancellation.
4. The source retrieves and normalizes the complete selected account state.
   Validation and summary preparation occur before commit.
5. The writer publishes material and progress together. The result identifies
   in-memory retention versus acknowledged durable persistence.
6. `App.status` loads a coherent data version and computes an owned summary. It
   never fetches, resolves secrets, or writes. The CLI renders the result.
7. `prepare` reads one version through `LearningStore::load` and its explicit
   grammar file, calls `prepare_context`, then renders the complete result.
   It constructs no source/runtime, holds no writer, and saves no classification.

One current store instance/directory is an explicit account scope. It must not
switch to a different account after its first successful write. This is not yet
a Cloud tenancy model. A future Cloud request authenticates and authorizes its
learner/connection before calling the library; do not infer authorization from
an arbitrary caller-supplied learner ID.

For generation, extend the flow explicitly:

```text
request -> settings and secrets -> dependencies -> optional explicit sync
        -> selected learner progress -> LearnerKnowledgePolicy
        -> language material and examples -> candidates -> validation
        -> accepted result or typed failure
```

`App` can accept all principal dependencies and construct `ExerciseGenerator`
internally. A convenience builder selects a documented default composition;
advanced users can compose the same dependencies directly. The standalone
generator accepts already prepared knowledge and does not require persistence.
Generation neither implicitly synchronizes nor changes learner progress.

Shared clients/pools/stores live as long as the application. A request owns its
learner selection, immutable inputs, budget, and result. Read a coherent input
version and pin prompt-set identity/version for each generation; release database
transactions before awaiting the model. New source data affects later requests.

## Generation structure and extension boundaries

The following is the target responsibility model, not G0's implementation list:

```text
LearnerKnowledge + GenerationRequest
                -> GenerationPlan
                -> CandidateGenerator
                -> Candidate[]
                -> Analysis + required checks
                -> Combined review when required
                -> Selection
                     -> accepted exercise
                     -> bounded repair -> NEW candidate -> full reassessment
```

Keep `LearnerKnowledge` and `LearnerKnowledgePolicy`; the discussion's
`KnowledgeProfile` and `KnowledgeProfileBuilder` do not introduce parallel
responsibilities. Knowledge about a vocabulary use keeps its written form,
reading, and meaning associated. A provider grammar entry and a recognizer that
can detect it are different facts; unsupported recognition is explicit and does
not imply equivalence across providers.

The logical text and analysis structures are:

```text
Candidate (immutable identity, content, generator provenance)
  GeneratedText
    Sentence -> text
    Story -> optional title + Paragraph[] -> Sentence[] -> text

TextAnalysis (candidate identity, analyzer/dictionary versions)
  ParagraphAnalysis[]
    SentenceAnalysis[]
      tokens + morphology + proposed readings
      grammar matches + complexity measurements
```

These diagrams need not become one public Rust struct per line. Choose enums
where distinct content variants require distinct payloads; introduce dialogue
or poetry variants only with an actual supported use case. Fragments may refer
to ranges in one immutable text representation. Tokens are analysis products,
not authoritative content supplied by a generator. Sentence segmentation and
other derived boundaries must also be attributed to their producer, not treated
as independent linguistic evidence merely because they have a type.

Generate a story as a coherent whole. Apply checks at their meaningful scopes,
including multi-token expressions and whole-story coherence. Titles also need
the applicable learner constraints. Findings carry a candidate reference and an
unambiguous location; token indices additionally identify the relevant analysis.
Document whether ranges use UTF-8 bytes or another unit. Editing creates a new
candidate (optionally linked to its parent), followed initially by full analysis
and assessment. Incremental invalidation is deferred.

Separate execution from judgment. A completed check has pass/fail/inconclusive
and findings; an execution failure remains a typed error, and a skipped check is
reported as not run. Required outcomes gate acceptance; quality measurements
rank eligible candidates. Keep weights and thresholds in explicit selection
configuration. Do not invent calibrated confidence from an arbitrary LLM score.
Measure false acceptance/rejection, unresolved cases, success before repair,
cost/latency, and learner-reported reading friction when evaluating real text.

Group components by concern while using small contracts for roles. For example,
a vocabulary component may contribute generation restrictions, check a candidate,
and propose repair information. An analyzer or candidate generator may implement
only its own role. Ordinary functions suffice for fixed deterministic behavior;
traits need a demonstrated substitution. Avoid a global context/service locator,
optional-hook mega-trait, execution DAG, or plugin manifests.

Trusted extension logic returns typed task contributions or uses explicitly
provided capabilities. The orchestration owns phase execution and the shared
model budget; no independent model calls hide in validators or retries. Prompt
composition consumes typed inputs plus replaceable prompt content and decodes
validated response shapes. This does not require a heterogeneous task registry
or arbitrary schema-merging framework. Role-specific contracts and their exact
Rust ownership/dispatch choices are verified in the implementing slice.

### Implemented minimal adaptation for G0

```text
CLI argument parsing -> structured word/grammar inputs + requested count
                    -> synchronous library preview
                    -> deterministic selection -> independent checks -> report
```

`src/preview.rs` exposes `preview(words, grammar, take)` with concrete
`WordEntry` inputs, a borrowed `Preview` result, `PreviewChecks`, `CheckOutcome`,
and typed `PreviewError`. Private functions validate all supplied inputs and
check the produced selection. Borrowed slices retain order, duplicates, and
associations without copying strings. A temporary `HashSet<&WordEntry>` checks
complete-entry membership with expected O(n + k) entry operations for n supplied
and k selected entries, plus string hashing cost and O(n) auxiliary storage.
`Hash` and equality are derived over all three fields. The set serves lookup
only; the original slice supplies ordered output with duplicates intact.
An independent length check compares the result with the requested count.
Grammar and linguistic correctness are explicitly unassessed.

CLI delimiter parsing and rendering stay in the executable. Rendering applies
`str::escape_debug` to word fields and grammar descriptions, keeping declarations
on one line and control sequences visible without mutating the library inputs.
A private data-dir resolver serves sync/status/prepare, but not preview.
Transient preview input never passes through `LearningStore`, `WaniKaniSyncData`, an account-scoped `App`, or a
knowledge policy. There is no generator/checker substitution need in this slice:
unit tests exercise the actual private checker with deliberately invalid data.
No trait, future text type, registry, or fake backend was added. Existing
sync/status behavior, cache schema, adapter layout, and package boundary remain.

## Implemented offline preparation slice

Its explicit flow is:

```text
CLI -> one LearningStore read + explicit grammar-file read
    -> synchronous prepare_context(source, grammar, policy, targets)
    -> knowledge derivation + structured lexical retrieval -> explanatory report
```

The standalone operation borrows the already-loaded WaniKani version and manual
grammar declarations. The selected concrete `LearnerKnowledgePolicy` derives
knowledge without fetching, saving, reading the environment, or consulting the
clock. The result preserves policy and source evidence, target associations,
grammar assertions, and unassessed linguistic limits. Source-subject eligibility
must not be represented as proof of every reading/sense combination.

Grammar declarations belong to learner data; their separate versioned JSON input
does not change WaniKani schema 1. One-based declaration IDs are scoped to the
loaded input, not a cross-edit or cross-provider ontology. File loading is an
explicit adapter operation, while derivation and retrieval remain synchronous
deterministic library logic.

The existing `LearningStore` provides the demonstrated file/memory substitution.
One lexical source and concrete policy alternatives do not justify a new trait.
Do not retrofit G0, add a material-store hierarchy, or introduce a registry.
Retrieval uses associated source fields without claiming linguistic validation;
source examples are not automatically safe practice passages.

`grammar.rs` owns validated manual declarations. `knowledge.rs` owns concrete
policy choices and borrowed `LearnerKnowledge` evidence. `preparation.rs` exposes
`PracticeTarget`, `PreparedContext`, retrieved target evidence, typed errors, and
explicit unassessed aspects. It indexes available vocabulary by exact word, then
matches accepted reading/gloss fields without synthesizing combinations. Multiple
matches are errors before eligibility is considered. Input order and duplicates
survive selection; source examples remain ordered and unfiltered. Private helpers
validate target fields and match one source record. There is no new storage port,
policy trait, generator, or checker abstraction.

## Stores, source data, and consistency

| Backend | Retention and failures | Verification |
| --- | --- | --- |
| In memory | Lives with its explicit store handle; no crash durability; full replacement and account validation still apply | Real state transitions, failed writes, reader/writer isolation |
| Local file | Complete replacement under an advisory lock; failures before replacement preserve the old cache; directory-sync failure means replacement happened but durability is uncertain | Actual temporary files, interruption and filesystem faults |
| Future SQL | Transactional material/progress publication scoped to learner/connection; database errors and commit uncertainty remain explicit | Isolated real database, transactions and concurrency |
| Future API | Latency, authentication, availability, write support, and consistency depend on the service contract | Real HTTP adapter against a local controlled endpoint |

Do not imply that an in-memory backend verifies file durability or SQL behavior.
Do not silently substitute memory storage after a persistent-store failure.
The current cache remains `wanikani.json` with the schema-1 `snapshot` key; the
Rust type name does not determine its wire format.

Multiple future connections can provide the same material category. Preserve
connection-level progress provenance. Refreshing one connection replaces only
its own data, preserving other connections and manual declarations. Report each
connection's outcome and data age; partial success is not an all-source success.

Separate updates from derived views. `LearnerKnowledge` starts as an on-demand
calculation. A materialized view, if later justified, carries input revisions
and policy version. CQRS does not require event sourcing, a command bus, or a
second database. Generating an exercise is a costly operation even without a
database write; fetching an existing result is a different operation.

## Generation composition examples (future Ruby pseudocode)

One-off use does not need a learner store:

```ruby
knowledge = LearnerKnowledgePolicy.default.derive(progress)
candidates = PromptedCandidateGenerator.new(model: model, prompts: prompts)
generator = ExerciseGenerator.new(
  materials: InMemoryLanguageMaterialStore.new(materials),
  examples: InMemoryExampleSearch.new(examples),
  candidates: candidates
)
result = generator.generate(knowledge, request, budget)
```

Cloud uses public adapter code with private runtime data:

```ruby
store = PostgresLearningStore.new(pool)
prompts = FilePromptStore.new(directory: private_prompt_directory)
candidates = PromptedCandidateGenerator.new(model: model, prompts: prompts)

app = Yomibu::App.new(
  materials: store.materials,
  progress: store.progress,
  sync_writer: store.sync_writer,
  sources: authorized_sources,
  examples: PostgresExampleSearch.new(corpus_pool),
  knowledge_policy: LearnerKnowledgePolicy.default,
  candidates: candidates
)
report = app.sync(authorized_learner) if refresh_requested
result = app.generate(authorized_learner, request, budget)
```

These are composition sketches, not frozen Ruby-like constructor requirements
for Rust. In-memory data does not imply an offline model. Replacing the model or
the corpus adapter must not change generation logic. A thin Cloud client calls
the remote Yomibu use-case API; it does not masquerade as a `LanguageModel`.

## Scenario checks and implementation timing

| Scenario | Composition change | Shared behavior and timing |
| --- | --- | --- |
| Local CLI, remote model | Local data/prompt stores plus a remote `LanguageModel` | Knowledge derivation, context preparation, validation, and budgets remain the same; implement with the first generation slice |
| Local CLI, local model | Replace only the model adapter, with an explicit capability/structured-output contract | Same generation flow; implement a local adapter only when a specific runtime is selected, without pretending all models have identical capabilities |
| Local algorithm, remote corpus | Replace `ExampleSearch` or the needed material read capability with an HTTP adapter | Selection and validation remain local; network failures and deadlines remain explicit; remote corpus is deferred |
| Thin remote Yomibu client | Use a Yomibu use-case client calling the Cloud API | The server owns generation dependencies; this is an alternative execution boundary, not a model swap; remote protocol is deferred |
| One-off in-memory generation | Pass progress/derived knowledge and material values directly; use memory adapters only for required lookup contracts | Same standalone generator without persistence, learner registration, or synchronization; cover with the first generation slice |
| Several sources of one category | Explicit connection IDs and selected progress for each learner; share provider clients where safe | One connection's refresh never overwrites another; policy combines preserved evidence; connection model and merge rules precede implementation of multiple providers |

Adapter substitution preserves the consuming algorithm only when its contract
is met. A read-only remote corpus cannot implement a writable synchronization
store, and volatile storage cannot promise crash durability. Do not hide these
differences behind a uniform success value or automatic fallback.

## Public contracts and evolution

The implemented storage entry points are `App.new(store).status()` and
`App.new(store).with_source(source).sync()`. Manual preview has the separate
`preview::preview` entry point described above. `SyncReport` contains an owned
summary, source synchronization times, and persistence classification. Errors
retain the source/storage type. In particular, a file
`WriteError::DurabilityUncertain` must remain distinguishable through `SyncError`;
the caller can inspect the visible cache instead of blindly retrying retrieval.
No result is persisted by status. Sync may create the directory/lock file before
fetching, but does not replace usable data after a source failure.

The current storage methods are synchronous for memory and local files. Do not
implement remote or asynchronous SQL adapters by blocking inside these methods.
Design their async read/publication capabilities with the actual backend in the
later storage milestone. The library is unpublished and its Rust API is not yet
stable; preserve schema-1 disk compatibility independently of source API changes.
There is no frozen generic source ontology or promise of drop-in asynchronous
storage for this first slice.

Future generation results must identify the accepted exercise, validation
outcome, policy/input/prompt revisions, and consumed attempts/model usage when
known. Distinguish invalid constraints, unavailable context, rejected candidates,
model/transport failure, deadline/cancellation, and exhausted budget. Successful
generation does not imply saving a result or modifying learner progress.
Avoid automatic retries after uncertain model completion; Cloud idempotency and
explicit result persistence need their own documented contract.

## Files, packages, and repositories

The current implementation slice is organized by responsibility:

```text
src/
  lib.rs                     public entry points
  main.rs                    CLI composition and rendering
  app.rs                     sync/status orchestration and reports
  domain.rs                  retained source data and invariants
  summary.rs                 deterministic source summaries
  preview.rs                 synchronous manual selection, validation, and checks
  grammar.rs                 validated manual familiarity declarations
  knowledge.rs               on-demand policy decisions and borrowed evidence
  preparation.rs             exact cached lexical retrieval for explicit targets
  ports.rs                   source and atomic storage capabilities
  adapters/
    mod.rs
    grammar_file.rs           explicit read-only versioned JSON input
    sources/
      mod.rs
      wanikani/              HTTP client, private DTOs, boundary tests
    stores/
      mod.rs
      in_memory.rs           real volatile storage
      file/
        mod.rs               file store adapter
        cache.rs             existing validated persistence and locking
        cache/tests.rs       filesystem fault/interruption tests
tests/                       use-case, store-contract, CLI and integration tests
examples/preview.rs          runnable direct manual preview
examples/prepare.rs          runnable direct preparation from synthetic values
ARCHITECTURE.md              this design
SPEC.md                      product contracts
PLAN.md                      implementation stages and evidence
```

As working functionality arrives, split `domain.rs` into `domain/learner.rs`,
`materials.rs`, `progress.rs`, and `exercise.rs`; add `knowledge/` and
`generation/`. Put model adapters in `adapters/models/`, prompt adapters in
`adapters/prompts/`, and corpus adapters in `adapters/examples/`. WaniKani stays
under `adapters/sources/wanikani`; Bunpro joins that category when implemented.
Create files for cohesive responsibilities, not one file for each field/type.

Keep code in one public GitHub repository initially, including source adapters,
SQL adapters, and potentially the Cloud host. Add a workspace package for a real
deployable Cloud binary or independently consumed client when build/dependency
boundaries justify it. Do not split packages merely to mirror every module.
Private prompt sets and corpora can live in a private asset repository or store
with separate access/deployment; their generic readers need not be private.

Public basics remain available. Future proprietary prompt variants should stay
private from creation, because prior publication cannot be undone by deleting a
file. Credentials and learner data are never repository assets. Public/private
visibility does not replace licensing: provider content retains its provenance
and access restrictions. WaniKani commercial content use requires separate
resolution before Cloud launch; API access is not an open content license.
See the [API content restrictions](https://docs.api.wanikani.com/20170710/#respecting-subscription-restrictions)
and [terms](https://www.wanikani.com/terms), reviewed on 2026-09-30.

## Testing and operational contracts

Use strict Red–Green–Refactor for new behavior. Exercise real domain and
application code with in-memory inputs; substitute only the external behavior
needed by a test. Scripted model responses are test support, not a production
fallback or evidence of linguistic quality. Keep real HTTP/file tests and later
real SQL tests. Share contract cases for common invariants, and retain separate
backend tests for different durability and failure semantics.

No universal `create_null()`, mock framework, or runtime test flag is required.
Check observable outcomes and important effects: cost limits, credential
boundaries, account isolation, and persistence. Internal method-call sequences
are not the public contract.

Document deadlines, cancellation, retries, partial outcomes, data age, and
commit uncertainty. Existing WaniKani request limits remain; a total refresh
deadline/collection bound is still a separate unresolved enhancement. Never
automatically repeat a costly model request whose completion is uncertain.
Generation must retain typed validation failures and must not silently weaken
constraints. Diagnostics identify operations and versions without leaking tokens,
private prompts, or learner content.
