# Yomibu implementation plan

`SPEC.md` is the authoritative product and architecture baseline. This document
tracks delivery order and acceptance criteria; it does not authorize future
milestones merely by listing them. `ARCHITECTURE.md` records responsibilities,
composition, and file/package/repository boundaries.

## Library navigation by capability — 2026-10-07

Grouped the live story stages under `crates/yomibu/src/story/`. Open `mod.rs`:
the adjacent `plan_generation` and `generate_story` functions show validation,
selection, exact request preparation, full-inventory assessment setup, provider
execution, assessment and result in order. Stage calls point directly to sibling
`request.rs`, `selection.rs`, `model_request.rs` and `assessment.rs`. Private child
modules use ordinary public re-exports, preserving existing `yomibu::story` imports.

Grouped evaluation result types/check outcomes in `evaluation/mod.rs`, supported
constructions/morphology/grammar observations in `structure.rs`, and full-inventory
lexical checks in `inventory.rs`. Moved the shared single-use projection from story
into evaluation, removing evaluation's dependency on story orchestration. Existing
public evaluation imports remain unchanged. Smaller cohesive modules stay flat;
there is no global steps/domain hierarchy, new wrapper, dependency or API skeleton.
CLI, retrieval, analysis, sync/status, fixtures, historical pins and lockfile are
unchanged. Current navigation/usage documentation and crate Rustdoc are aligned.

This is a behavior-preserving move verified with existing tests, not a new feature.
Focused story/provider tests passed after the first move; evaluation/story tests
passed after the dependency correction. Reviewed moved logic against the previous
checkout, including execution order, validation, prompt/request bytes, assessments,
outcomes and safeguards. Full verification: `cargo fmt --check`, locked strict
Clippy on all targets/features, `cargo test --locked --all` (182 Rust tests/doctests,
none failed or skipped), `git diff --check` and strict Rustdoc passed. All nine
Python dictionary-setup tests passed, setup verified the real pinned dictionary,
and the documented offline lexical retrieval/story-preview demonstrations passed.
No live model calls were made.

Explicit simplification review covered navigation, duplication, ownership, naming
and public API size. Kept concrete stage calls in the entry functions and local
helpers beside their callers; only `into_owned` gained parent-module visibility
for result assembly. No additional abstractions or ownership changes were justified.
Review and merge status is recorded by the PR rather than a future claim here.

## Thin CLI review follow-up — 2026-10-06

Greptile initially scored PR #19 3/5 and raised two valid public-boundary findings.
Confirmed RED separately: invalid cache save returned success, and a later
oversized input caused one HTTP call before rejection. GREEN validates cache
contents before temporary-file creation/replacement and validates all input sizes
before encoder work. Regressions cover unsupported versions, wrong dimensions,
unchanged usable bytes, no directory creation, zero HTTP calls for an oversized
33rd UTF-8 input, and accepted exact-limit input in ordinary [32, 1] batches.
Simplification review extracted one private size check shared by story embedding
input construction and public cache preparation. No new error layer or service
was warranted. Existing valid CLI behavior, size limits and request bytes remain
unchanged. Required checks and strict Rustdoc passed again after the fixes (182 Rust
tests/doctests, none skipped, with the real pinned dictionary). Current PR records
the resulting CI/re-review and merge status.

## Thin CLI, slice 3 — shared reports and executable layout — 2026-10-06

Completed all three approved slices without an intermediate approval pause.
`main.rs` now parses and chooses exit status; `args.rs` defines Clap inputs;
`commands/` configures paths/adapters/credentials/runtime and calls library use
cases; `output/` owns terminal escaping and layout. Story handlers still call
adjacent `story::plan_generation` and `story::generate_story`. No generation app
wrapper, new trait, dependency, provider behavior or API placeholder was added.

Moved existing wire projections and candidate-error/NotRun classification to
`reports::{story,candidate,analysis}`, with the existing report-boundary regression
migrated to the library. Added a concrete bounded explicit-file input adapter with
typed errors. RED confirmed its absent API; GREEN covers exact UTF-8 bytes, the
read limit, maximum limit arithmetic and no writes/directory creation. JSON
formats and process environment lookup remain CLI choices. Historical grammar
input is archived byte-for-byte under `docs/history/commands/`; current inventory
fixtures and historical A1/G1/G2 evidence remain unchanged.

The first complete test run exposed parser-safety assertions still using retired
commands. Migrated them to active candidate-count/knowledge-policy flags, confirmed
the focused test passes, then reran the complete suite. Full verification: 180
Rust tests/doctests passed, none skipped, using the real pinned dictionary; all nine
Python dictionary-setup tests passed. `cargo fmt --check`, strict locked Clippy on
all targets/features, `cargo test --locked --all`, `git diff --check` and strict
Rustdoc passed. Dictionary setup verified the local checksum-pinned bundle;
offline lexical retrieval/story-preview demonstrations passed. No live model calls.

Explicit simplification review covered ownership, naming, API size, duplication
and navigation. Reused one cache-file adapter instance, removed the unnecessary
optional prepared-cache conversion and its impossible error path, and kept
concrete stage calls. Shared reports only project results; neither rendering nor
conversion repeats assessment. Remaining command logic selects host resources or
formats input/output, and source eligibility, request preparation, retrieval
batching and assessment are library responsibilities. No further wrapper or
service extraction was justified. Exact current story request fixtures, hashes,
prompt/provider settings, evaluation source, original spans and lockfile are
unchanged. Documentation is aligned with the final layout. Review/merge status is
recorded by the pull request rather than an unverified future claim here.

## Thin CLI, slice 2 — retrieval and file cache — 2026-10-06

Moved missing-vector reuse, batching/merging and complete validation into
`retrieval::prepare_cache`, returning typed `EmbeddingError`. Moved bounded file
reads and synchronized atomic publication into `EmbeddingCacheFile`, including
explicit before-replacement and uncertain-durability outcomes. CLI selects paths,
encoder/credentials/runtime and calls these boundaries. No dependency or behavior
change is intended; the existing 32-input batching, cache format and byte limits
are preserved. RED confirmed the missing library APIs; GREEN: four boundary tests,
the migrated oversized-publication regression and all ten story CLI tests pass.
Tests cover complete-cache reuse, model changes, later-batch failure, unchanged
previous data, malformed files and absent parent directories. Simplification review
kept one preparation function and one concrete file adapter without a new store
trait or application wrapper. Documentation records the reusable call path.

## Thin CLI, slice 1 — prototype retirement — 2026-10-06

Retired old `preview`/`prepare`, their examples, library APIs, standalone grammar-file
adapter and obsolete first-N/source-inspection tests. Kept source eligibility and
migrated its three evidence/policy tests to `tests/knowledge.rs`. `derive(source)`
and `LearnerKnowledge` now have no grammar dependency; inventory calls the same
rules directly. RED confirmed old preview exit 0 rather than the intended parser
exit 2, and the new one-argument eligibility API initially failed to compile.
Historical purposes/contracts/code pin are in COMMAND_HISTORY; current story,
analysis, sync/status and source evidence contracts remain unchanged. CI demos now
exercise offline story retrieval and preview rather than retired commands.
GREEN: knowledge/inventory/grammar and CLI suites pass (18 tests). Simplification
review removed the unused grammar-file adapter and avoided replacing retired APIs
with compatibility wrappers; no full-source copies or changed eligibility rules.

## Shared library story workflow and A1 runner retirement — 2026-10-06

The approved follow-up moves reusable orchestration out of the CLI. Read adjacent
`story::plan_generation` and `story::generate_story` in `crates/yomibu/src/story.rs`:
offline validation → one selection → bounded immutable request → full-inventory
assessment projection; then one provider attempt → every candidate assessment →
owned result. CLI `generate_story_command` loads files/embeddings and initializes
dictionary/credential/client/runtime after planning, then renders and chooses exit
status. Preview calls only offline planning. Future API callers reuse these same
library functions; no API scaffolding, trait framework or dependency was added.

The complete immutable `StoryGenerationPlan` contains final selection, exact AI
request and assessment inputs. The former subset-only type is renamed
`StoryVocabularySelection`. `StoryGenerationResult` owns original candidates and
partial/completed assessments, with typed candidate errors; provider failure still
returns `ProviderError` without a result or retry. Assessment inputs borrow original
inventory/request and selected IDs independently of the selection struct, avoiding
self-reference and full-inventory clones. `Sentence` now uses borrowed/owned text,
loses `Copy`, and `text(&self)` borrows; only returned analyzed sentence text is
copied (at most 100 scalars). Ordinary analyzer behavior and serialization stay the
same. These breaking Rust APIs are documented in SPEC/current usage.

RED: the new workflow tests failed on the expected missing public planning and
execution functions before implementation. GREEN: three tests exercise exact
fixture bytes/hash, preflight limits/stale caches, one-attempt provider failure,
full-inventory departures versus failures, blank/oversized typed errors and owned
results surviving dropped inputs/resources. Existing story integration matrices
now call the shared workflow, including configurable counts, competing identities,
partial target completeness and the object-combination safeguard. Existing CLI,
provider, evaluation and real-analyzer suites protect remaining contracts.

Removed `examples/a1.rs`, its CI demonstration and its now-unused library
Clap/anyhow dev dependencies; the lockfile changes only those local dependency
edges. Packet-only runner contracts are retired. Useful judgment/error/NotRun/span
protections remain in current suites; the oversized-candidate partial-result case
now runs through shared story generation. Historical inputs/records remain
unchanged. A1_IMPLEMENTATION and GENERATION_HISTORY pin the last runner checkout
`d2adfcfe6dffede63363bf1a11be81ce8fe85606`; no private holdout was rerun/rescored and
the historical no-go remains. SPEC, architecture, Rustdoc and usage are aligned.

Simplification review kept adjacent concrete entry functions and existing lower-level
stages. Report conversion now accepts the complete plan/result rather than separate
candidate/assessment collections; no self-referential return type or duplicate
assessment enum was introduced. Analyzer comparison tests reuse available analysis
instead of repeating work, and ownership changes are limited to the returned result.
All fixture data, request bytes/hash, prompt/provider settings, limits, no-retry
behavior, reports, exit status and original UTF-8 spans remain unchanged.

Verification: `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**203 passed**, none failed/ignored),
`git diff --check` and strict Rustdoc passed. Nine dictionary-setup Python tests,
both remaining direct-library examples, root offline retrieval and story preview
passed; preview matches the exact request fixture/hash. Updated documentation
links resolve, and all fixture bytes are unchanged. Tests use the real pinned
analyzer/dictionary and local HTTP mocks. No live model calls or hosted/macOS CI
runs were made. An initial full run hit a sync subprocess ENOENT while separate
Cargo demonstrations rebuilt the executable; the complete rerun with no concurrent
builds passed. No unrelated sync implementation or test was changed.

## Library/CLI workspace — 2026-10-06

The user approved moving to a Cargo workspace before the API slice. Two members
now make the existing boundary visible: `crates/yomibu` is the reusable library;
`crates/yomibu-cli` depends on it and builds the executable named `yomibu`.
No `yomibu-api` directory/package or placeholder HTTP behavior is introduced.
Public Rust library paths and CLI behavior stay unchanged.

Moved library modules, adapter tests, 16 integration suites and three examples
into the library package. Moved executable modules/unit tests and six executable
integration suites (including combined sync coverage) into the CLI package.
Shared immutable fixtures remain under root `tests/fixtures`; analyzer tests
still load the real pinned bundle under root `target/a1/current`. Updated include
and dictionary paths explicitly, without fake analysis or copied fixtures.

The root manifest owns shared package metadata, dependency versions and the
unchanged SHA-256 test profile. Member manifests declare their own dependencies.
Clap/anyhow are CLI production dependencies; the preserved A1 library example
uses them only as development dependencies. Normal library dependency-tree
inspection confirms neither is present. The lockfile adds only the local CLI
package; existing external versions, sources, checksums and options are unchanged.
Root CLI/example commands still select their unique target, with one target
directory and one pinned toolchain/lockfile. Existing CI commands cover both
members without workflow changes.

Behavior-preserving moves first passed all 203 existing tests. Follow-up
verification caught Clap deriving `--version` from the new package name.
RED: an executable regression (including hostile argv[0] on Unix) confirmed
`yomibu-cli 0.1.0` instead of `yomibu 0.1.0`. GREEN: explicitly name the command
`yomibu`, retaining its existing trusted binary name, version and diagnostic
layout. No version/CLI contract change is intended. Rustdoc also confirmed a
same-name binary/library output collision; `doc = false` on the CLI binary keeps
public Rustdoc with the library, as before.

Simplification review kept the existing adjacent story stages and ordinary
command functions, rather than combining file moves with a new engine object,
service hierarchy or orchestration API. No public library API rename or extra
package is warranted. Updated SPEC, architecture, README/current usage and
current navigation links; historical generation documentation retains its pinned
code paths. All shared fixture data and current request bytes/hashes are unchanged.

Verification: `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**204 passed**, none failed/ignored),
`git diff --check` and strict Rustdoc passed. Nine dictionary-setup Python tests
also passed. Root CLI help/version/manual preview, all three direct-library
examples, offline preparation/retrieval and story preview passed; story preview
matches the exact existing payload/hash. All 73 Rust include paths and updated
current documentation links resolve. No live model calls or hosted CI runs were
made for this workspace move; macOS was not checked locally.

## Legacy G1/G2 retirement — 2026-10-06

The authorized cleanup leaves one generation path, beginning at
`story_command.rs::run_generation`. Removed the G1 `generate-candidates` command,
its binary composition/report module and G1/G2 public generation entry points,
fixed-pair/context types, authored situation selector and focused observations.
Unrelated preview/preparation/sync/status/analysis and dictionary behavior stays
unchanged. Current story request bytes/hash, prompt revision, provider settings,
limits, reports, full-inventory assessment, partial results, original UTF-8 spans,
A1 no-go and object-combination safeguard are unchanged.

Migrated provider tests before removal and verified them green through
`generate_story_candidates`: exact current request/settings/provenance,
whole-response rejection, redirects/credentials/no retry, deadlines and truncated,
chunked or close-delimited bodies. Migrated typed downstream report errors to
story tests, preserving available analysis and NotRun without completed judgments.
The credential subprocess regression now uses story inputs and the real pinned
dictionary; `.env` is ignored, invalid secrets stay out of diagnostics and no files
are written. Existing story/evaluation tests cover full-inventory membership,
original candidates, partial results, terminal presentation and object uncertainty.
Removed only obsolete focused selector, fixed-pair and legacy CLI/schema tests.

RED: the obsolete-command subprocess test still accepted `generate-candidates`;
confirmed the expected exit-0 versus exit-2 failure before removing dispatch.
GREEN: the migrated tests and current story suites pass after removal.
Simplification review removed the now-single-variant `GenerationError` wrapper;
`Client::generate_story_candidates` returns `ProviderError` directly. Removed the
never-emitted story `GenerationProvenance::focused_context` field and placed the
small target morphology predicate beside its only caller in `story.rs`.
`generation.rs` now contains only live candidate assessment/error and provider
metadata types. No new dependency, trait, framework or legacy Rust archive exists.

[Generation history](docs/GENERATION_HISTORY.md) records conclusions and exact
Git pins. Moved 15 non-Markdown artifacts byte-for-byte from the old test fixture
folders to `docs/history/generation/`; kept detailed G1/G2 usage clearly historical
and updated the comparison recipe to read the archive. The v1/v2 live comparison
remains **not run**, with no new linguistic-quality claim. SPEC, architecture,
README and current usage describe only the story implementation and explicit
breaking Rust API changes. All current story fixtures match the pre-cleanup pin.

Verification: `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**203 passed**, none failed/ignored),
`git diff --check`, strict Rustdoc and **9 dictionary-setup Python tests** passed.
Real analyzer/dictionary tests used the pinned installed dictionary. No live
model calls were made; dependency files are unchanged. Hosted checks/review are
recorded on the cleanup PR; native macOS was not run locally.

## PR #15 Greptile corrections — 2026-10-06

Read Greptile's 4/5 summary and all three inline comments on `8849125`.
All three findings were valid and addressed:

- Object grammar targets: confirmed RED (`unknown_object` was incorrectly
  `observed`), then shared the existing direct-object evidence predicate between
  checker and observer. Real-dictionary coverage includes false, missing,
  alternative and competing evidence, plus explicit true evidence. Unsupported
  constructions remain unassessable; object combinations remain Inconclusive.
- Embedding bounds: confirmed RED for 32 valid 17 KiB documents exceeding the
  encoded body cap, and for a later oversized combined document making an
  earlier provider call. The adapter now batches by actual JSON size, including
  escapes; common input preparation checks every complete lexical document
  before provider work. Cache publication still requires all batches to succeed.
- Restored `exact-reading request` in the unrelated preparation documentation.

Simplification review reused the checker's lexical predicate without changing
historical judgments, kept wire-size batching in the concrete HTTP adapter, and
kept one shared per-input limit. No dependency, trait or generic batching framework
was introduced. Updated SPEC, architecture and usage limits to match.
Verification: `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**237 passed**, none failed/ignored),
`git diff --check`, and strict Rustdoc all passed with the real pinned dictionary.
The object regression passed again after the final assertion/refactor review.
No live provider calls were made; request fixtures and dependency files are unchanged.

## Story naming and explicit assessment inputs — 2026-10-06

Approved breaking rename: `yomibu::story`, `StoryRequest`,
`StoryGenerationOptions`, `StoryGenerationPlan`, `AiModelRequest`,
`StoryCandidates`, `StoryCandidateAssessment`, and `StoryError` replace the current
reading API names. CLI names are `preview-story` and `generate-story`, without
aliases; `prepare-retrieval` is unchanged. Pronunciation readings are unchanged.
No new story-content requirements, dependencies or generation features were added.

`build_ai_model_request` returns `(StoryGenerationPlan, AiModelRequest)` so any
request-budget trimming is visible in the final plan. The AI payload owns only
bytes/hash and encoded options. `StoryAssessmentInputs::new` builds the separate
full-inventory assessment inputs before execution resources are initialized;
`assess_candidates` receives them explicitly. Returned candidates own their texts
and provenance with no borrow of the outgoing request. Reporting remains separate
from orchestration and assessment. Historical G1/G2 entry points and frozen
fixtures remain; their removal is a separate cleanup.

Current JSON kinds: `story_generation_plan`, `story_generation_plan_preview`, and
`experimental_story_candidates`; prompt revision `story-inventory-v1`. Current
fixture bytes/hash intentionally change with the kind value; the prompt's actual
instructions, model settings, limits and one-attempt behavior are unchanged.

TDD: updated CLI tests failed on unrecognized new command names and acceptance of
old names; the executable report test failed on the old JSON kind before changes.
Local mock HTTP tests required sandbox socket access; the real pinned dictionary
is used for assessment. Behavior-preserving ownership changes use existing tests.
Simplification review: removed request/candidate lifetime parameters and hidden
assessment state from transport, retained concrete adjacent stages and separate
report conversion, and named the existing projection `project_structural_inputs`.
No full-inventory clone, generic request framework or extra dependencies were needed.
The default request fixture is now 2,231 bytes, SHA-256
`40aa1a47429ef6a72de095f991476edef7019757cf4273cb3789706d0bf91d65`.

Verification after refactoring: `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**235 passed**, none failed/ignored),
`git diff --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`
passed. The renamed retrieval script parses successfully. HTTP tests use local
mock servers; analyzer tests use the real pinned dictionary. No live model calls,
native macOS checks or hosted CI were run. Dependency files and historical G1/G2
request fixtures are unchanged.

## Reading orchestration and execution options follow-up — 2026-10-06

The user requested a higher-level entry body and configurable candidate count,
then clarified that output intent and execution options should be separate.
`run_generation` now consists of concrete stage calls: load/validate inputs,
load/prepare embeddings, select vocabulary, prepare the request, load the analyzer,
request candidates, assess, report, and determine execution status. Cache branches,
credential handling and Tokio event-loop construction are nearby private helpers.
No generic pipeline, new trait, runtime-owning library API or service hierarchy
was introduced. Existing executable tests passed after the extraction.

`StoryRequest` retains brief/targets. New `StoryGenerationOptions { candidate_count }`
defaults to 2; preview and generation accept `--candidates N`. Per user preference,
there is no arbitrary 4/8 ceiling. Counts must be positive and `512 × N` must fit
checked arithmetic before embedding/dictionary/credential work. Actual provider
token limits and the existing 64 KiB response cap still apply, so large counts
can fail explicitly. There is one request, without automatic splitting/clamping
or retry. Repair-round settings are deferred until repair exists.

API changes (names updated by the later story follow-up): `build_ai_model_request` takes an explicit fourth `StoryGenerationOptions`
argument; current reading results expose a string slice, assessments return a
vector, and reports contain every candidate plus separate `generation_options`.
The fixed-pair G1/G2 APIs/fixtures remain unchanged; they share the same bounded
HTTP transport. Before the story naming follow-up, the prompt revision was `reading-inventory-v2`. Its default
request fixture was 2,222 bytes, SHA-256
`340e0fbae60cd88b6a424479e8825392c91ebca18648b3738aaaeb9616649bc6`.

TDD: first confirmed failures for missing options support and the unrecognized
`--candidates` flag. Tests now cover 1, 3, 8 and 9 returned candidates, exact
prompt/schema/token count, unchanged prepared bytes, too few/many response items,
retained original texts and partial failures, default count, no arbitrary cap,
zero count and arithmetic overflow before resource work. The changed current
fixture was reviewed and updated; legacy fixtures remain byte-identical.
Simplification review kept ordinary functions and one small options value,
removed fixed-pair assumptions only from the current path, and kept async details
inside concrete I/O helpers. Tokio local variables are named `io_runtime`.

Follow-up verification on Linux with the real pinned analyzer/dictionary:
`cargo fmt --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**235 passed**, none failed/ignored), `git diff --check`,
and `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps` all passed. Existing
G1/G2 request fixtures and the dependency manifest/lockfile are unchanged. Native
macOS and hosted CI were not run here.
No API key was configured and no live provider call was made. A configured Cloud
secret would enable the still-pending hosted embedding comparison; automated tests
continue to use loopback HTTP and the real pinned analyzer/dictionary.

## Shared reading implementation — 2026-10-06

This is the current change after merged PR #14 (`9ceba44`). The dated sections
below remain historical records. Branch: `codex/shared-reading-inventory`.

Implemented the approved breaking design in one package, without dependencies,
service hierarchy or a generic pipeline. `LearnerInventory` joins explicit
manual input and the existing WaniKani policy projection. `StoryRequest` carries
a free-form brief and multiple vocabulary/grammar target IDs. One common story
path ranks a cached plan, freezes provider bytes, generates once, then assesses
all original candidates against the full inventory. The later story naming follow-up
separates outbound request ownership from explicit assessment inputs (see above).

Current CLI: `prepare-retrieval`, `preview-story`, `generate-story`. Old
`generate-focused`/`context-preview` commands and permissions/focus-entry flags
are removed, without aliases. The new prompt revision and JSON contracts are
intentional changes. Historical G2 library operations, v1/v2 request fixtures,
comparison recipes, A1 frozen evidence and unrelated command behavior remain.
The current entry is `story_command::run_generation`; library stages are
adjacent in `story.rs`, presentation separate in `story_command/report.rs`.
See [usage and migration](docs/STORY_GENERATION.md).

The one `Embedder` port has local/hosted HTTP implementations and an explicit
nonsemantic lexical baseline. Flat model-specific vector caching validates shape,
finite/nonzero values, exact input/model identity and complete responses.
Missing vectors never trigger implicit hosted work. Preview is offline and
constructs no dictionary/credential/client/runtime. Embedding preparation is
explicit; generation's final request preflight precedes its dictionary and
credential initialization. Cache publication preserves prior data on batch or
pre-publication failure and distinguishes post-publication durability uncertainty.

### TDD and simplification record

The initial source projection, request/ranking, adapter and CLI tests were run RED
before implementation (missing APIs/commands), then GREEN. Focused regressions
also reproduced and corrected these issues during the new-path review:

- Impossible selection wrote vectors before failing: selection bounds now fail
  before embedding work.
- Duplicate document keys hid an invalid returned vector: every returned vector
  is validated before cache deduplication.
- Unrelated OOV evidence hid an observed target: observation and completeness
  are separate, including `partial` in JSON and text.
- Competing source alternatives or unsupported morphology could claim a target:
  conservative observations retain uncertainty.
- Grammar observation accepted malformed object/copula structure: bounded shape
  recognition now rejects it.
- Structural projection discarded competing word alternatives and incorrectly
  treated direct-object evidence as unambiguous: only unique single-use forms
  enter that projection; the full inventory remains the lexical boundary.
- Combined vocabulary findings inherited a particle coverage label: current
  inventory coverage is explicit for every outcome.
- Oversized cache publication could produce an unreadable replacement: a
  pre-publication size check retains the prior file, tested with a small limit.

Existing behavior-preserving composition changes use the existing tests; no
artificial behavioral test was added merely to move code. The final simplification
review retained concrete adjacent stages, separated rendering, reused existing
provider transport/checks, and avoided cloning the full inventory. Small owned
embedding metadata simplifies lifetimes. Duplicate-form counting uses one local
map rather than a quadratic scan. No extra analyzer/generator trait was justified.

New coverage includes equivalent manual/WaniKani prepared bytes, multiple targets,
full-inventory assessment, real Sudachi spans, object uncertainty, exact new request
fixture/hash, offline preview, terminal escaping/layout, both candidate texts and
partial results, cache reuse/429 preservation, opt-in and removed-command errors.
The real checksum-pinned analyzer/dictionary is used in all analysis cases;
loopback provider mocks only supply transport responses, never fake analysis.

### Evidence and remaining prerequisite

The public six-brief retrieval probe ran with the explicit lexical baseline:
mean recall@3 **0.4722**, approximately 11–24 ms preparation per case in one debug
run. This is not useful evidence for selecting a semantic model. The local default
endpoint had no service and no hosted credential was configured; **local dense
and hosted comparisons remain unrun**, with no implicit substitute/default.
[RETRIEVAL](docs/RETRIEVAL.md) records the corpus, recipe, measurements and limits.
No live generation call or linguistic acceptance is claimed. A1's no-go and
object-combination safeguard remain unchanged.

Final verification on Linux/x86_64 with the pinned Rust toolchain:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed.
- `cargo test --locked --all`: **231 passed**, none failed or ignored, including
  four doctests and real pinned dictionary tests.
- `git diff --check`: passed, including newly added files.
- `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`: passed.
- Dictionary setup verified the existing checksum-pinned files; nine offline
  Python installer tests passed.
- Manual fixture preview and WaniKani-plus-manual CLI preparation/preview smoke
  passed with cleared environments and explicit lexical-baseline retrieval.
- Cargo manifest/lockfile, toolchain pin, historical focused fixtures and A1
  fixtures remain unchanged. New request fixture: 2,224 bytes, SHA-256
  `7243bf73c97265e786d26b7f5b83a13fa0f64fb188827b85cb4f9e350c5deb76`.

Native macOS and hosted CI were not run here. The existing CI matrix is unchanged.
The dense retrieval comparison is the only remaining external evaluation
prerequisite in this slice; no quality/default-model claim is made for it.

Rust notes for a Ruby developer: data is passed directly between named functions.
Borrowing keeps the request/inventory alive through assessment without copying
whole collections; a small result wrapper ties the generated pair to those exact
inputs. `?` propagates stage errors, while candidate errors remain values so both
results can be reported. Async is confined to provider I/O; the CLI owns runtimes.

## Current state and stopping point

Milestone **1d — Milestone acceptance is complete**. The macOS/Linux CI workflow
enforces the committed lockfile and repository toolchain. Public APIs, dependency
features, credentials, and errors have been reviewed; the durability error now
exposes its I/O cause. Quality gates passed locally on macOS/arm64 and in an
isolated Linux/arm64 container, then on GitHub-hosted Ubuntu/x86_64 and
macOS/arm64. Follow-ups add dependency caching, avoid redundant runs, and overlap
independent HTTP test scenarios while strengthening exact timer checks.
On 2026-10-01 the user authorized the architecture migration for existing
`sync/status` described in `ARCHITECTURE.md`. That migration is complete; its
native macOS verification is recorded below separately from historical CI runs.
Generation and other later product milestones remain outside this scope.

**G0 — Manual candidate preview is complete** in the subsequent 2026-10-01
Cloud implementation. The synchronous public library operation, thin CLI,
independent checks, and runnable direct-library example are delivered below.
The original six TDD cycles, review follow-ups, and verification are recorded in
the G0 section.
The next offline learner-constraints/retrieval slice was approved on 2026-10-02
and its implementation and real-learner acceptance are complete as of 2026-10-03.
The dated acceptance record below supersedes the earlier unverified status.
Validated generation remains unimplemented; the separately approved G1 experiment
is recorded below. A1 implementation was explicitly approved on
2026-10-03 after the protocol-only stage. The bounded synchronous analyzer,
concrete adapter, and thin synthetic evaluation example are now implemented.
Engineering verification and the visible evaluation are recorded below;
the single held-out run and private scoring are complete. **A1 is complete with
a no-go for this implementation.** The 24-development/12-challenge references are
frozen and the visible set has run. Private holdout was not authored here; a later comment
exposed linguistic hints, with the repair recorded below. The user
operates the approved external blind review manually; model-only judgments stay
provisional, and no model call was made by the agent.
All 36 original cases and one focused clarification have been audited. Two
original positives remain unresolved; revision 2 replaces both affected pairs
before freezing, with complete history preserved. Implementation-side source
review covers 24 core outcomes and 12 challenge references after the four replacement
reviews were reconciled. All active visible references are provisionally supported.
The 2026-10-04 custodian freeze receipt reports all 24 current holdout v2 cases
reviewed and reconciled, with five exposed families replaced and history retained.
Combined reported core reference coverage is 48/48, including the 24 visible
references; all remain provisional. Only the released held-out input was processed
here; hidden answers and source ledgers remain in separate custody.
The [Scope clarification](docs/A1_SCOPE_CLARIFICATION.md) confirms that structural
coverage does not mirror permission results; any check corrections stay in
custody before freeze. The reported reference gate is met. The visible run matches
all 24 development outcomes; a small TDD reporting-span fix also gives 12/12 exact
negative reason/span matches. Three unsupported challenge Pass results prevent a
bounded go. Required locked gates passed after the fix. Code/configuration is now
frozen and the released input ran once offline after hash verification. All 24
cases and 120 checks completed without execution errors. The returned private
scoring receipt reports all held-out targets met: 24/24 outcomes, 120/120 check
judgments, and 11/12 exact negative reason/span matches. One nested-span discrepancy
is preserved; no reference or implementation changed for scoring. The three
unsupported challenge Pass results determine the no-go. No further review round
or rerun is required to complete this investigation. See
[evaluation status](docs/A1_EVALUATION_STATUS.md).

The user subsequently authorized a separate code revision for the exposed
unsupported Pass results. Local checkpoint `f1237d4` preserves completed A1 on
`codex/a1-offline-analysis`; `codex/a1-unsupported-pass` continues from it.
The [follow-up record](docs/A1_FOLLOWUP.md) documents the conservative object-use
restriction and its reduced visible coverage. The scored holdout is not rerun,
and the completed A1 results and no-go remain unchanged.

On 2026-10-04 the user authorized a separate **offline analyze CLI** milestone.
It exposes the existing bounded analyzer/evaluator through explicit input and
dictionary paths, without changing linguistic judgments. Its contract, TDD and
verification record follow the A1 safeguard section below. A1's historical
no-analysis-CLI scope does not restrict this new authorization.

The user then approved **G1 — experimental single-sentence candidates** for
implementation, local testing, documentation and a draft review PR. G1 adds one
explicit provider attempt and local assessment of both candidates; it does not
authorize purchases, live calls, accepted exercises or another linguistic review.

On 2026-10-05, G2 adds focused offline selection, optional request preview and
one-command experimental generation, now merged in PR #9. The lexical-boundary
follow-up changes the focused prompt to v2 and prepares a bounded v1/v2 comparison,
with all required local checks passing. Paid calls remain pending authorization.
Historical G2 Linux/macOS verification and dictionary-setup results remain below.

On 2026-10-05 the user approved managed dictionary loading after a separate plan.
Implementation and local Linux verification are complete: explicit fully verified
offline import, atomic generation publication, managed mapping and strict external
owned loading share one analyzer. The delivery, TDD, safety contract and controlled
release measurements are recorded at the end of this document.

The 2026-09-30 naming follow-up adopts the design vocabulary in `SPEC.md` and
renames the existing sync-data type. It does not start a new product milestone.

On 2026-09-27, the official Rust release page and `rustup update stable` both
confirmed Rust 1.98.1. Installed the exact 1.98.1 toolchain with rustfmt and Clippy,
and pinned it in `rust-toolchain.toml`, using edition 2024. The repository pin
selects Rust/Cargo 1.98.1 without changing the user's global 1.82.0 default.
`Cargo.lock` records the resolved dependencies. In 1b, reqwest 0.13.5, Tokio
1.53.1, and tempfile 3.27.0 support HTTP and persistence; wiremock 0.6.5 is a
development dependency. Proptest remains deferred.

## Focused generation readability refactor — 2026-10-06

Approved after a read-only plan against checkout `03a6f6a`. The command sequence is
now discoverable in `src/focused.rs::run_generation`: bounded input, selection,
request preparation, execution-resource initialization, generation, assessment,
report conversion/rendering, and execution status. Preview has its own offline
entry point without an optional dictionary execution-mode argument.

Focused presentation moved to `focused/report.rs`; shared G1/G2 candidate DTOs and
rendering moved to `candidate_report.rs`. The additive library
`GeneratedCandidates::assess_focused` / `FocusedCandidateAssessment` API combines
existing assessment and observations, removing domain work from report construction.
Selection, provider settings/transport, prompt bytes, evaluator behavior, A1 history,
and unrelated commands are unchanged. No new dependencies or generation features.

These are behavior-preserving moves verified with existing cases, not behavioral
changes requiring an artificial RED test. The real focused-assessment integration
cases now exercise the aggregate library method; the synthetic evaluation-error
boundary test moved with observation composition into the library. Existing G1
report conversion tests remain at the shared presentation boundary.

Simplification review: retained direct stage calls and small explicit preparation
sequences in each command, colocated file/credential helpers with orchestration,
and kept report construction free of assessment. Inputs and generated strings
remain owned locally; assessments and report views borrow them without added
clones or a self-referential aggregate. Existing lower-level public APIs remain.
The existing client constructor seam still only selects the concrete loopback
adapter in tests. No additional abstraction was justified.

Verification on Linux with the pinned Rust 1.98.1 toolchain:

- `cargo fmt --check` — passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `cargo test --locked --all` — passed: 215 unit/integration/example tests and
  3 compiling doctests, with no failures or ignored tests.
- `git diff --check` — passed.

Initial sandboxed test runs could not bind local HTTP mock ports; rerunning with
local network permission resolved that restriction. Focused suites passed after
report extraction and after assessment composition moved. Analysis tests used the
real pinned dictionary at `target/a1/current/system_core.dic`, including external
and managed loading paths. Direct source comparison confirmed that shared candidate
DTO/conversion/rendering and existing assessment/observation bodies are unchanged.
No live model call, private evidence review or scored holdout rerun was performed.

## Vertical milestones

| Step | Status | Deliverable | Acceptance |
| --- | --- | --- | --- |
| 0 — Decisions | Complete | Three authoritative, consistent documents | Sourced external facts; proposed structure; Rust rationale; no application initialization |
| 1a — Offline status | Complete | Package, domain snapshot, cache reader, real `status` command | Fixture-backed integration tests; no token/network dependency; useful errors and summaries |
| 1b — First complete sync | Complete | HTTP adapter, normalization, safe persistence, thin CLI composition | Mock API → normalized snapshot → disk → offline status succeeds through real components |
| 1c — Sync resilience | Complete | Expanded failure-path verification and hardening of the 1b safety foundations | Failure cases preserve a usable complete cache; concurrent access and post-replacement errors behave as specified |
| 1d — Milestone acceptance | Complete | macOS/Linux CI and reviewed public library surface | Local and hosted macOS/Linux gates pass; documented limitations; no placeholder future features |
| Architecture migration | Complete | Library `App`, explicit source/storage contracts, file and in-memory stores, adapter layout, architecture document | Both stores run real sync/status; existing safety and schema contracts preserved; no generation placeholders |
| G0 — Manual candidate preview | Complete | Synchronous library operation and `preview` CLI, structured word entries, deterministic selection and independent checks | Direct and CLI calls agree, no storage/network/runtime requirements, explicit assessment limits; all 85 tests and required gates pass on Linux/x86_64 |
| Offline preparation slice | Implementation and real-learner acceptance complete, 2026-10-03 | Local grammar input, revisable knowledge policy, explicit targets, cached lexical evidence | Synchronous CLI/library agreement, explainable decisions, typed failures, unchanged schema-1 source data; private real-learner acceptance completed; linguistic validity unassessed |
| A1 — Bounded offline analysis evaluation | Complete — no-go | Pinned real adapter, synchronous bounded checks, thin example; frozen visible/held-out evaluation and private scoring | Held-out targets met; 3 unsupported challenge Pass results fail the safeguard; one exact-span discrepancy retained; no accepted exercises |
| Offline analyze CLI | Complete | `analyze --dictionary PATH --input PATH [--json]` with bounded ordinary input | 13 real-adapter CLI tests; CLI/library agreement, all completed outcomes, original spans/provenance, safe presentation, explicit errors and offline isolation; required locked gates passed |
| G1 — Experimental sentence candidates | Complete for review; user-run live smoke passed | Explicit single OpenAI attempt, immutable pair, independent pinned local assessment and safe reports | Local HTTP/real-analyzer tests and required gates passed; user-supplied dog/cat smoke report; partial-result preservation; no exercise acceptance |
| G2 — Focused experimental context | Merged in PR #9 | Deterministic selector, offline optional preview, selected-only bounded transmission and separate focus/context evidence | Full Linux/macOS CI passes with real pinned dictionary; 190 Rust tests per platform; fresh cloud dictionary setup resolves the initial download restriction; no live call or linguistic acceptance |
| G2 lexical boundary | Implemented; comparison not run | Compiled v2 selected-content-word restriction and fixed v1/v2 comparison recipe/artifacts | 195 local Rust tests and required gates pass; all three current comparison requests/manifest checked in CI; offline release request reproduction verified; paid calls require separate approval |
| Managed dictionary loading | Implemented; locally verified | Explicit verified offline import/update, mapped managed startup and fully verified external snapshots | 217 locked Rust tests and nine installer tests pass on Linux; release CLI measurements below; macOS/hosted verification not run for this revision |

Implementation steps use small Red-Green-Refactor cycles (see `AGENTS.md`). Tests
accompany behavior, beginning with a confirmed failing test, rather than being
added after implementation or postponed to 1c.

Basic credential protection, timeouts, and safe persistence apply as soon as the
respective I/O is introduced; 1c completes and exercises the failure paths rather
than retrofitting unsafe foundations.

## SHA-256 test-profile optimization — 2026-10-05

Started `codex/test-sha2-profile` from freshly fetched `origin/main`
`fce266e882febf764a468c17b9c127c89e3ec766`, containing merged PR #10.
The checkout had no user edits; the existing worktree, branches and ignored
`target/` artifacts were preserved. No reset, stash, clean or worktree removal
was used. Read AGENTS, SPEC, ARCHITECTURE and this plan before editing.

### Profile investigation and preserved verification

`cargo test --locked --all --no-run -vv` builds the library, integration tests,
ordinary CLI (`src/main.rs --crate-type bin`) and binary test harness
(`src/main.rs --test`) under the **test** profile. Before the change they shared
one unoptimized `sha2` artifact. After the change all linked the same new artifact,
compiled with `-C opt-level=3 -C debuginfo=2 -C debug-assertions=on`.
The integration tests launch `CARGO_BIN_EXE_yomibu`; G1/G2 binary child helpers
and the Sudachi isolation probe relaunch `current_exe()`. Thus a test-only
package override reaches both executable boundaries without a dev override.
Yomibu and other dependencies keep their existing optimization levels.

Added only `[profile.test.package.sha2] opt-level = 3`. `sha2` remains locked at
0.10.9 with default/std features: its source selects software SHA-256 on aarch64
without `asm`, while this x86_64 host has SHA-NI. No feature/dependency change
or new hardware requirement is introduced. The adapter still checks exact length
and SHA-256 of all 217,466,039 bytes before constructing Sudachi with
`Storage::Owned(bytes)`. The dictionary hash remains
`53fa281d11eef3769712fe1c3c892117338f9892bee6daf4dad51daa5281bb6f`.
Analyzer/configuration pins, release configuration, Cargo.lock, all source/tests,
CI runners/gates/cache/provider settings, frozen A1 records, prepared comparison
fixtures/manifest/recipe and v1/v2 code pins remain unchanged.

### Before/after measurements

Baseline measurements completed **before editing Cargo.toml**. Both configurations
ran sequentially on the same Linux/x86_64 cloud machine (AMD EPYC 9V74, five
visible CPUs), pinned Rust/Cargo 1.98.1, dictionary bundle and cached dependency
sources. No Rust flags/wrapper/target or test-thread override was set. Used
separate fresh build directories `/tmp/yomibu-ci-sha2.ZNCgrU` and
`/tmp/yomibu-ci-sha2-changed.ySzmft`, preserving existing artifacts. Commands were
identical apart from `CARGO_TARGET_DIR`; no competing build/test ran during timing:

```sh
# Set CARGO_TARGET_DIR to a fresh directory for each configuration.
time -p cargo test --locked --all --no-run -vv
time -p cargo test --locked --test analyze_cli  # first prebuilt execution
time -p cargo test --locked --test analyze_cli  # second prebuilt execution
time -p cargo test --locked --all
```

| Local measurement | Baseline | sha2 test opt-level 3 |
| --- | --- | --- |
| Fresh full prebuild, shell wall time | 81.07s | 81.51s |
| analyze_cli execution 1, libtest / shell wall | 72.80s / 73.14s | 5.96s / 6.29s |
| analyze_cli execution 2, libtest / shell wall | 72.20s / 72.45s | 5.90s / 6.10s |
| Full suite, summed libtest execution / shell wall | 202.31s / 203.89s | 28.93s / 30.26s |

Each representative run passed all **15** tests; both full suites passed **195**
including two doctests, with zero failures/ignored tests. Warm Cargo build checks
took 0.14–0.23s, separately visible in logs. Summed libtest execution excludes
compilation, Cargo/process overhead and doctest compilation; child-process time
is already included in its parent test. Raw local logs remain under ignored
`target/ci-sha2-profile-2026-10-05/`.

The representative mean fell from 72.50s to 5.93s (about 92%); broader execution
fell about 86%. Fresh compilation was essentially unchanged, slightly slower in
this pair. This supports the narrow configuration change, without a compilation
speedup claim, benchmark infrastructure or timing assertions. Linux hardware
hashing and hosted runner variation prevent these numbers establishing a macOS
improvement on their own.

### Workflow, refactor review and checks

This profile-only change is non-behavioral configuration under AGENTS.md: recorded
before/after performance evidence replaces an artificial failing functional test.
Explicit REFACTOR review covered simplification, duplication, naming, modelling,
ownership/borrowing and idiomatic Rust. Kept the single package override and
existing owned verified bytes, real CLI processes and same-size wrong-checksum
regression. No production/test refactor, cache, bypass or new abstraction was
justified. The changed representative executions and full suite passed after
this review. SPEC and ARCHITECTURE contracts require no revision.

Documented `python3 scripts/setup_a1_dictionary.py` verified the existing complete
pinned bundle and both notices without a download; all nine offline installer
tests passed. Initial default-sandbox Git access could not reach the proxy;
fetch succeeded with the authorized command network capability. Rust tests used
that capability for isolated loopback mocks, with no paid provider/live WaniKani
call, learner/private evidence access or held-out rerun.

All required local checks passed on pinned Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all` — 195 passed, none failed or ignored
- `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`
- `git diff --check`
- `git diff --exit-code -- Cargo.lock`

A separate `cargo build --locked -vv` passed and confirmed ordinary development
builds still compile/link unoptimized sha2 (the original artifact metadata),
without `-C opt-level=3`. No release settings were added or changed. Full diff
review confirmed only Cargo.toml and developer/evidence documentation changed.

### Hosted CI and draft delivery

Committed/pushed `7eba14c42de06e52c6ee7f2ce87e4ab7d70f6cc9` and opened
[draft PR #11](https://github.com/kalorz/yomibu/pull/11).
[CI run 37328750596](https://github.com/kalorz/yomibu/actions/runs/37328750596)
passed **every gate on Linux/x86_64 and macOS/arm64**. Both checkout logs confirm
that exact pushed head in tested merge `356df10` with base `fce266e`.
Each passed 195 Rust tests (including two doctests; none failed/ignored), nine
installer tests, real pinned dictionary setup, all synthetic demonstrations,
formatting, strict Clippy/rustdoc, whitespace and unchanged-lockfile checks.
Both logs include the real dictionary mismatch and comparison-manifest regressions.

Inspected two earlier unoptimized PR #10 runs, retaining both:
[37293738878](https://github.com/kalorz/yomibu/actions/runs/37293738878) at
`98e3df9` and [37296324473](https://github.com/kalorz/yomibu/actions/runs/37296324473)
at `9da9f0a`. The former matches the reported approximately 7m19s macOS /
3m51s Linux jobs. The latter is a faster macOS baseline before this change.
All three runs used the same Ubuntu 24.04 and macOS 26 arm64 image versions and
Rust 1.98.1. Earlier runs restored exact cache matches; changed jobs restored
the previous dependency cache via fallback (`full match: false`), with no caching
configuration change.

| Run / OS (seconds) | Dictionary setup | Test compilation | Summed test execution | analyze_cli | Test step | Job wall |
| --- | --- | --- | --- | --- | --- | --- |
| Earlier baseline / Linux | 5.67 | 17.48 | 167.63 | 63.71 | 185.83 | 230.90 |
| Earlier baseline / macOS | 5.10 | 23.58 | 353.32 | 153.05 | 380.38 | 438.53 |
| Later baseline / Linux | 5.15 | 17.56 | 167.62 | 64.49 | 185.95 | 228.34 |
| Later baseline / macOS | 4.48 | 16.87 | 266.03 | 111.90 | 285.48 | 332.79 |
| Changed 7eba14c / Linux | 6.58 | 11.85 | 19.71 | 3.04 | 32.18 | 88.19 |
| Changed 7eba14c / macOS | 4.93 | 27.80 | 55.69 | 18.74 | 88.48 | 177.02 |

Compilation comes from Cargo's Finished-test summary; execution sums libtest
durations. Step/job wall times use log boundaries, excluding queueing. Dictionary
setup includes the nine installer tests. Doctest compilation and process/Cargo
overhead remain in the test step, outside the summed execution column.

Observed macOS execution fell about 79% relative to the later baseline (about
84% relative to the earlier one), while test compilation **increased 10.93s**.
Its job fell from about 5m33s to 2m57s. Linux execution fell about 88%; its
Clippy step rose from 3.60s to 22.33s with fallback cache reuse, despite faster
test compilation. Setup remained a small part of both jobs. This is evidence of
an observed macOS CI improvement, separately from the controlled local Linux
comparison. Hosted machines/load were not controlled: the two macOS baselines
already differ substantially. No fixed runtime or compilation improvement is
promised. Ordinary development-profile demonstrations still pay their unchanged
dictionary startup cost. Final documentation-head CI is recorded on the PR.

Rust note for a Ruby developer: Cargo profiles select compiler settings, not
dependency versions. A package override optimizes that crate's hashing while
keeping application debugging and runtime ownership intact. A test-built CLI
uses the test profile even though it is an ordinary executable; child processes
do not automatically rebuild it or change its profile.

## G2 lexical boundary follow-up — 2026-10-05

Started new branch `codex/g2-vocabulary-boundary` from fetched `origin/main`
`818dda5` (merged PR #9). Verified both G2 `565be18` and focus-report fix
`562cc11` are ancestors. The clean checkout had only ignored `target/` artifacts;
no reset, clean, stash or continuation of the merged branch occurred.

Changed only the compiled focused prompt to `g2-focused-sentence-v2`: selected
entries bound content words, each candidate uses the focus, supports remain
optional, and inflections/grammatical forms require explicit bound grammar.
Readings/senses guide intended use without validation claims. Request ordinary
Japanese without extra content words or forced variation. Treat JSON/descriptions
as data and remove the suggestion of unseen permissions. G1, all provider settings,
limits, selection, exact-byte preparation, evaluator and full original permissions,
focus status/completeness, object safeguard and frozen A1 evidence are preserved.

RED: revised the canonical request fixture/assertions before production behavior.
`cargo test --locked --test focused_request canonical_v2_request` ran one test
and failed on revision `g2-focused-sentence-v1` versus expected v2. An earlier
incorrect exact-name filter ran zero tests and is not RED evidence. GREEN: changed
the focused prompt/revision and current generation/executable revision assertions;
all four request tests passed. Canonical v2 is 2,312 bytes, SHA-256
`a3cbfc09ec6cb2b1264737a0ba97e90367644e91b151a0689f6276d2c4b43422`;
independent Python and `wc`/`sha256sum` agree. Preserved the 1,882-byte v1 fixture
and all earlier measurements below as history.

Explicit REFACTOR review covered concision, duplication, names, modelling,
ownership/borrowing and idiomatic Rust in production/tests. Kept one compiled
constant using `concat!` for readable instruction groups, existing immutable
request and shared transport, and the existing contract tests. No further code
change, abstraction or dependency was justified. Post-review focused reruns passed:
request 4, CLI 4, lexical evidence 6, real-dictionary generation 4, and binary
focused tests. These establish transmitted bytes/report semantics, not better
Japanese. The documented dictionary setup verified the existing real pinned bundle;
all nine offline installer tests passed, without downloading another dictionary.

The [comparison recipe](docs/G2_COMPARISON.md) fixes three public synthetic inputs,
two repetitions/version, 12 alternating attempts, exact bodies/hashes, original
permissions, separate report dimensions and a refreshed official-pricing estimate.
Paid calls remain unauthorized; no live comparison or improvement claim exists.

### Local verification and comparison preparation

Pinned Rust 1.98.1 on Linux/x86_64 passed all required checks:
`cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**194 tests including two doctests, zero failures or
ignored tests**), `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`,
`git diff --check`, and `git diff --exit-code -- Cargo.lock`. The ordinary suite
used real pinned Sudachi plus isolated loopback HTTP mocks, with explicit command
network capability. No external provider request or private evidence was used.
A test-list audit confirms 194 entries; it is not another test run.

Comparison preparation built baseline/current code in isolated checkouts and
ran all six offline previews against the same three fixed inputs. Independently
checked all exact request bytes/hashes, then ran the documented offline recipe
with isolated pinned release builds successfully. Decoded request comparison confirms only
developer prompt content differs. Predetermined order is two repetitions of
pet-rest, pet-walk and book-reading, each v1 then v2, for at most 12 attempts and
24 candidates. The existing output limit remains 1,024 tokens per attempt.
The 2026-10-05 official pricing refresh estimates US$0.0454656 total before tax
using a conservative input allowance; this is not an enforced billing cap.
Raw results are planned under ignored `target/g2-comparison/2026-10-05-v1-v2/`.
The comparison is **not run**: explicit paid authorization is pending and no
`OPENAI_API_KEY` is available in this environment. No adherence improvement is
claimed. Recipe syntax/links and frozen/provider/evaluator preservation were
reviewed; final-head Linux/macOS CI results will be recorded on the draft PR.

Rust note for a Ruby developer: `concat!` joins the instruction groups at compile
time, so this readable source still supplies one immutable string, without a
runtime prompt framework. Borrowed full inputs and owned prepared bytes continue
to keep generation restrictions separate from evaluation permissions.

### PR #10 Greptile coverage review — 2026-10-05

The summary and inline comment identify one valid, nonblocking coverage gap:
the canonical pet-rest snapshot was checked automatically, while the comparison's
pet-walk/book-reading requests and manifest metadata were checked only during
offline preparation. Added one deterministic request test for all three current
comparison inputs. It verifies the input/request paths, exact input lengths and
hashes, selected situation, prompt revision, exact regenerated request bytes and
request lengths/hashes against the manifest. Historical v1 requests and both
frozen comparison code pins remain unchanged.

This is a test-coverage extension of existing behavior; no production behavior
changed and no artificial RED was required. The focused request suite passed all
five tests. Explicit REFACTOR review covered duplication, naming, modelling,
borrowing and idiomatic Rust: shared the existing input parser, kept fixture
metadata checking local to the test, and found no production refactor warranted.
Passing fixture checks establish reproducibility, not model adherence. No paid
comparison calls were made.

Post-review Linux verification on pinned Rust 1.98.1 passed formatting, locked
strict all-target/all-feature Clippy, locked full tests (195 including two
doctests; zero failures or ignored tests), strict rustdoc, whitespace and unchanged
lockfile checks. Real pinned analyzer tests and isolated loopback mocks passed;
Linux/macOS CI for the pushed coverage commit is recorded on the PR.

## G2 — focused experimental context, 2026-10-05

Implemented the [G2 contract](docs/G2.md) on `codex/g2-focused-context`, created
from freshly fetched `origin/main` at `9e74618f17cd74ea7e0dd35d6ee5914a2cb436d1`
without pruning, resetting, cleaning or stashing. The cloud checkout was clean;
the planning snapshot's pending Mac documentation patch was absent. The user
explicitly authorized reconstructing its reading-goal/abandoned-REPL amendments
from the pasted plan. No claim is made to have copied the unavailable Mac patch
byte-for-byte. Existing ignored artifacts and all frozen A1 records were preserved.

Normal use is one `generate-focused` command with per-invocation model opt-in;
`context-preview` is optional and genuinely offline. Deterministic selection is
separate from permissions. One immutable bounded request retains borrowed full
inputs for independent real analysis/evaluation, and focus/context observations
never rewrite evaluator findings. Comfortable, enjoyable reading and comprehension
remain goals for later voluntary learner feedback, not properties proved here.
No REPL, preview import, retained session, automated data bridge, new dependency,
live model call, private evidence access, holdout rerun, purchase or merge occurred.

### Observed Red–Green–Refactor evidence

1. Input bounds/checked focus test initially failed with unresolved
   `generation_context` import. Implemented full-inventory bounds and positive
   one-based identity; the focused tests passed. Reviewed validation order,
   ownership and naming; no further refactor was justified at that increment.
2. Pet-rest test failed on missing selection/report operations. Implemented exact
   focus-first selection, borrowed membership and exclusion reasons; all three
   initial tests passed. Independently corrupted produced selections to verify
   rejection of membership/focus/slot/order/size errors. Reviewed modelling; kept
   construction private and references borrowed rather than cloning permissions.
3. Remaining-situation/ambiguity tests failed with one decision instead of three
   and cat selected despite a competing sense. Added the fixed situations,
   alternatives and ambiguity rules. Six selector tests passed. Refactor review
   grouped same-spelling entries once to avoid quadratic duplicate comparisons;
   kept the three original definitions private without a selector framework.
4. Request tests failed on missing `prepare_focused_request`. Implemented pure
   selected-only preparation preserving every grammar description/binding. Tests
   passed for exact/one-over request limits, escaping expansion, and a 6,000-entry
   inventory larger than 64 KiB with an unchanged bounded request. Refactored the
   fixed G1 body construction into one shared helper; rechecked G1 wire contracts.
5. Adapter tests failed on missing `generate_focused_candidates`. Shared the
   existing transport/parser and retained original evaluation inputs. Two focused
   loopback tests passed. Default sandbox loopback binding was denied; reran with
   explicit network capability. Refactor review kept one transport/parser and
   local optional provenance, without an additional abstraction. Raw-socket G2
   deadline/truncation/chunk-bound coverage subsequently passed as well.
6. Focus/context report tests failed on missing observation functions. Implemented
   enum states, whole-unit/stem spans, component ambiguity, precedence and separate
   full-inventory membership. Four explicitly labelled structural/report-boundary
   tests passed. Reviewed POS indexing and slicing: structurally validate before
   reporting; only existing reading/stem helper visibility changed in evaluation.
   Real pinned-analyzer integration tests were present but unverified in
   the initial cloud run because the dictionary prerequisite was unavailable (below).
7. Executable preview test failed with `unrecognized subcommand context-preview`.
   Implemented both commands, bounded input and text/JSON reports. Four executable
   tests passed, including exact file size, malformed input, hostile controls,
   Japanese readability, streams/exits/help, cleared environments and poisoned
   ambient files. Refactored shared G1 provenance/candidate rendering, preserving
   its text shape; binary boundary tests passed for preview/client isolation,
   output failures and surviving valid analysis after a typed evaluation error.
8. Full-scope review caught generation text expanding every unselected label. A
   regression test failed because the private unselected sentinel was printed;
   generation now groups exclusion IDs/reasons, while preview expands labels for
   inspection. The regression and all three focused binary boundary tests passed.
   Reviewed selection, request immutability, error precedence, escaping, borrowing,
   naming and duplication again; no further change was justified. The canonical
   request is recorded as 1,882 bytes with SHA-256
   `81415d76fb7f43ba4cc435bc7d98afbe29abdff6cd33d7104b62ad14c63cb503`.

### Initial cloud verification and explicit prerequisite failure

Used pinned Rust 1.98.1 and a fresh temporary build directory
`/tmp/yomibu-g2-target.NZ8mQx`, retaining the existing ignored build artifacts.
The user explicitly authorized documented dictionary setup in this cloud after
confirming the Mac path was unavailable. `python3 scripts/setup_a1_dictionary.py`
failed at the network boundary with `Tunnel connection failed: 403 Forbidden`:
the environment allowlist omits `sudachi.s3.ap-northeast-1.amazonaws.com`.
The network restriction was reported and an allowlist update requested. No
alternate dictionary, simulated analyzer, implicit download or skipped test
substituted for this prerequisite.

Executed `cargo fmt --check`, locked all-target/all-feature Clippy with warnings
denied, locked rustdoc with warnings denied, `git diff --check` and lockfile
preservation checks. These passed. Selector/request/observation/preview tests,
concrete loopback adapter tests and real raw-socket tests passed.
`cargo test --locked --all` failed at dictionary-dependent binary tests; a
subsequent `--no-fail-fast` run completed the remaining targets to expose all
prerequisite failures. Eight targets failed (33 tests) on the absent dictionary:
the binary, analyze CLI, evaluation, focused generation, G1 CLI, OpenAI integration,
Sudachi and A1 example. These failures are not waived or counted as green.
macOS checks were not run locally. At this local checkpoint real pinned-dictionary
verification remained pending; the subsequent hosted results below resolve that
engineering-verification gap without substituting an analyzer or waiving tests.

Reviewed unchanged Cargo dependencies/lockfile, toolchain and analyzer/dictionary/
configuration pins, `App`, stores, knowledge policy and preparation. Evaluator
changes are only `pub(crate)` visibility for reading/stem helpers. Preserve A1's
historical no-go, 24/24 outcomes, 120/120 judgments, 11/12 exact negative reason/span
matches and the object-combination safeguard. New tests are engineering evidence,
not revised A1 results or validated Japanese.

### Hosted verification and draft delivery

Committed implementation `565be18d55a79f2e0e1c6829d31e60fcd51c0bfb`, pushed
`codex/g2-focused-context` and opened attached [draft PR #9](https://github.com/kalorz/yomibu/pull/9).
The repository's unchanged [CI run 37271053313](https://github.com/kalorz/yomibu/actions/runs/37271053313)
passed on both `ubuntu-latest` and `macos-latest`. Each runner explicitly installed
and checksum-verified the real pinned dictionary. Each passed **190 Rust tests,
including two doctests, with zero failures or ignored tests**, plus the dictionary
setup tests, strict formatting/Clippy/rustdoc, synthetic demonstrations, diff and
lockfile checks. This includes G2 real candidate composition, full-permission
comparison, original Fail spans, partial results, morphological occurrence and
object-safeguard tests, as well as unchanged G1/analyze regressions.

Documentation-only follow-up `fa148e0f660940ac74b42cdcc6f224a9008e16c1` recorded
these results; implementation and test sources were unchanged from the verified
commit. The local publisher-host restriction remained at that checkpoint, and
was not presented as a local full-suite pass. No dictionary was retrieved from CI
into the restricted workspace. The fresh-environment follow-up below records the
later setup and CI results. No live provider call, merge or purchase occurred;
the PR remains a draft.

### Fresh cloud setup and CI retry — 2026-10-05

Continued draft PR #9 at `fa148e0f660940ac74b42cdcc6f224a9008e16c1` after
inspecting the clean Git state and fetching without pruning. The fresh checkout
initially selected `work` at the main baseline; fetched and checked out the existing
`codex/g2-focused-context` branch. No reset, clean, stash, feature recreation or
new PR was used. Existing ignored `target/` artifacts were preserved.

The attached environment configuration version
`cecfgver_6ac344de04748193a49ad6d61146786e` reports desired and observed spec
revision 2, current observations and the allowed publisher host
`sudachi.s3.ap-northeast-1.amazonaws.com`. Its policy state still reports
`unknown`, which does not establish enforcement. Separately, the supported
version-1 `/etc/codex/network-policy.json` startup snapshot lists that host in
the restricted HTTP egress rules. The explicitly authorized
`python3 scripts/setup_a1_dictionary.py` succeeded through the inherited proxy
and configured CA trust with TLS verification enabled. It verified the pinned
archive and all three bundle files, then published
`target/a1/current/system_core.dic` with both publisher notices. Verified cache
reuse and all nine offline setup tests also passed. No policy bypass, alternate
artifact or credential was needed.

Inspected the current [CI run 37271779785](https://github.com/kalorz/yomibu/actions/runs/37271779785)
for `fa148e0`. Its original Linux job passed. Original macOS job `111640121477`
passed formatting, Clippy and nine setup tests, then failed at download with
`<urlopen error [Errno 54] Connection reset by peer>`; its Rust tests did not run.
Reran that failed job. Replacement macOS job `111647390013` installed the real
pinned dictionary and passed every quality step, including **190 Rust tests,
two of them doctests, with zero failures or ignored tests**. The retained Linux
result also passed 190 tests. The successful retry supports a transient transport
failure; review of the installer bounds, verification and publication safeguards
found no justified production change or refactor. The downloader and CI remain
unchanged.

Local Linux/x86_64 verification used the repository's exact Rust 1.98.1 pin,
installed into temporary tool storage because the fresh image lacked Rust, and a
fresh `CARGO_TARGET_DIR=/tmp/yomibu-g2-verification.z5VRdq`. All required checks
passed: `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo test --locked --all` (**190 passed, zero failed or ignored**, including
two doctests), `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`,
`git diff --check` and `git diff --exit-code -- Cargo.lock`.

This follow-up updates verification documentation only. Dependencies, lockfile,
toolchain, analyzer/dictionary/configuration pins, evaluator safeguards and frozen
A1 records remain unchanged. Final-head CI verification is recorded in draft
PR #9 after this follow-up is pushed. No live OpenAI/WaniKani call, purchase,
private evidence access, holdout rerun or merge occurred.

### Focus report review follow-up — 2026-10-05

Following the user's request to fix Greptile's reporting concern, amended G2's
aggregate-status contract. Unrelated OOV/unsupported morphology now leaves a
compatible focus `observed`; the separate `FocusCompleteness` enum reports
`complete`, `partial` or `not_run`. Focus-specific uncertainty still takes
precedence, and unknown lexical evidence cannot establish `absent` when no
occurrence was observed. Counts, spans, uncertainty reasons, evaluator judgments,
request behavior and contextual reading/sense limitations are preserved.

Observed RED: the unrelated-OOV regression returned `unassessable` instead of
`observed`; completeness assertions found a missing JSON field. GREEN: separated
focus-specific uncertainty from overall completeness, and all six lexical
boundary tests passed. A separate text-rendering RED showed the missing
completeness line while retaining escaped Japanese readings and original spans;
adding the line made all four focused presentation/boundary tests pass.

Refactor review covered precedence, uncertain absence, component ambiguity,
ownership, duplicate state and terminal rendering. Extracted the focus renderer
into one private function and used `concat!` for readable exact-layout assertions.
Reran the focused tests; no further production abstraction was justified. Added
real pinned-Sudachi cases for unrelated OOV/unsupported morphology and uncertain
absence, plus executable JSON/text coverage with loopback HTTP, unchanged exit
behavior, Japanese readability, hostile controls and exactly one request.

Verification in this older cloud instance used pinned Rust 1.98.1 and
`/tmp/yomibu-g2-target.NZ8mQx`. Formatting, strict locked all-target/all-feature
Clippy, strict rustdoc, nine offline dictionary-setup tests, diff whitespace and
lockfile/pin/evaluator preservation checks passed. The full locked offline Rust
suite was run with `--no-fail-fast`: 160 tests passed and 34 failed at the missing
dictionary prerequisite across eight targets, with none ignored (excluding the
nested failing child-process result printed by the Sudachi test). This instance
still reports the original restricted configuration without the publisher host;
the successful setup in the separate fresh session above remains historical
evidence, not a local pass for this fix. Real-adapter and executable verification
for the pushed fix is recorded in PR #9's Linux/macOS CI results.

Rust note for a Ruby developer: a serialized enum makes completeness an explicit
finite state instead of asking callers to infer it from status strings. The
renderer borrows the existing report; observation remains synchronous and never
feeds back into evaluation or another provider request.

## G1 — experimental single-sentence candidates, 2026-10-04

Started `codex/g1-experimental-candidates` from verified current `origin/main`
`cdb8ef5da04c89798738ac2bafb5c843dcd8fb7d` (PR #7 merge). `git ls-remote` agreed
with the local remote-tracking reference; the previous checkout was
`codex/offline-analyze-cli`, not main, and its tracked tree matched the baseline.
Preserved existing branches, commits and ignored files. All builds use a fresh
temporary `CARGO_TARGET_DIR`; the existing pinned dictionary is read explicitly.
The delivery fetch confirmed the same main commit. Repository `fetch.prune=true`
removed a stale PR #7 remote-tracking reference; restored that reference at the
verified PR head `3d79aa808a500eb50810f985a2449b936f764b08`. No local branch or
commit was removed. Subsequent fetches for this work must specify `--no-prune`.

Implemented the approved [G1 contract](SPEC.md#g1--experimental-single-sentence-candidates)
and [usage/privacy/spending/smoke documentation](docs/G1.md). The concrete adapter
sends at most one request, disables redirects/proxies/protocol retries, bounds
request/response bytes and deadlines, and keeps credentials out of error output.
The model supplies only text, never permissions or judgments. Both texts are
assessed independently by unchanged pinned Sudachi and evaluation, retaining
completed results beside execution errors. The existing offline analyze output
and environment/runtime boundary are preserved through shared presentation helpers.

### TDD and refactor record

| Cycle | Observed RED | GREEN and explicit refactor review |
| --- | --- | --- |
| Binding preflight | Focused test did not compile because `EvaluationBindings::validate` was absent | Extracted existing checks without changing order; invalid analysis still precedes binding errors in `evaluate`. Focused rerun passed; reviewed naming/ownership and added no new binding type. |
| HTTP request foundation | Real local-server contract test did not compile because `adapters::openai` was absent | Concrete one-attempt adapter sends exact explicit data/schema/settings and records the hash of those same bytes. Focused test passed. Reviewed defensive parsing, bounds and secret-safe typed errors; no SDK, model trait or dependency needed. |
| Independent assessment | Real HTTP-to-Sudachi test did not compile because assessment/error types and `assess` were absent | Fixed pair retains each text, typed errors and full direct-library-equivalent analysis/evaluation. Focused tests passed. Removed unnecessary input accessors; original declarations/bindings remain borrowed. |
| Real socket deadlines | Deadline test did not compile because a private duration constructor was absent | Factored client construction with unchanged production limits; stalled headers/body, disconnects, short bodies and streamed-size tests passed. Reviewed socket cleanup and attempt counts; no production timeout override added. |
| CLI opt-in/preflight | Production subprocess rejected the unknown `generate-candidates` command | Required opt-in/paths, strict bounded input and validation order passed executable tests, with no provider request. Reviewed separation of environment/runtime and library behavior. |
| CLI reports | Test subprocess composition did not compile because the shared entry was absent | Same entry accepts an explicit concrete loopback constructor only from the binary test harness. Real provider/analyzer reports, partial results, safe Japanese/control presentation, status and error tests passed. Extracted only bounded-read/check-rendering/safe-JSON helpers; existing analyze regressions passed. |

Additional acceptance tests cover malformed/count/type/extra-field/refused/incomplete
responses, additive metadata, optional usage, exact request/response bounds, zero
requests on preflight failure, redirects and HTTP errors without retry. They extend
the tested defensive foundation, not separate claimed RED cycles. Direct adapter
tests compare complete analysis/evaluation values, including empty permissions,
duplicates, 100/101-scalar boundaries and hostile text. Executable tests retain
the visible ordinary-object uncertainty at `6..24` and permission Fail spans
`3..6`, `9..12`, `18..24`. Typed analysis/evaluation error-report cases are explicit
boundary tests, not claims of inducing those failures in real Sudachi.

Refactor review covered production/test names, ownership, duplication, public APIs
and Rust idioms after the cycles. Clippy identified a large enum variant; boxing
the completed Evaluation reduced stack size without changing report contents.
No additional framework or public configuration type was justified. No tests read
real credentials, mutate global environment, substitute tokenizers or skip a
missing pinned dependency. All HTTP services used in verification are local mocks.

### Verification and delivery

Native macOS/arm64, pinned Rust 1.98.1, with a fresh temporary build directory passed:

- `cargo fmt --check`
- `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`
- `cargo test --locked --offline --all`: 158 test entries plus two compiled
  rustdoc examples, all passed, none ignored. This includes all 15 existing
  analyze CLI regressions, 7 OpenAI integration tests, 2 raw-socket tests,
  4 production generate CLI tests and 5 binary G1 tests (one subprocess helper).
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --no-deps`
- `git diff --check` and `git diff --exit-code -- Cargo.lock`
- Diff comparison against `origin/main` confirmed unchanged Cargo manifest/lock,
  toolchain, analyzer/dictionary/configuration pins, A1 historical documents,
  frozen public fixtures and research harness.

Local HTTP tests used authorized loopback sockets. Dependencies and linguistic
analysis stayed offline. No Linux/container or hosted CI result is claimed in
this local record; the existing workflow will run for the draft PR. Delivery is
on `codex/g1-experimental-candidates` for draft review against main, without merge.

No live provider call,
account purchase, billing-setting change, private evidence/learner read, holdout
rerun, frozen-reference edit, linguistic review or merge is authorized/performed.
Provider compatibility remains unverified until a separately authorized smoke.
The [one-call synthetic smoke proposal](docs/G1.md#proposed-live-smoke--requires-separate-authorization)
uses the dog/cat fixture and a US$0.01 allowance, not a guaranteed provider cap.

A1 remains complete with its historical no-go, 24/24 outcomes, 120/120 judgments
and 11/12 exact negative reason/span matches. The object-combination safeguard
and reduced coverage remain intact. Integration success is not repaired evidence.

Rust notes for a Ruby developer: `[String; 2]` makes candidate count part of the
type after parsing. Borrowed immutable permissions prevent a result from silently
substituting new rules; assessments borrow their original strings without a
self-referential object. Enum variants and typed errors distinguish completed
judgments from failed execution, while `Box<Evaluation>` keeps the result enum
compact without cloning evidence or creating a second evaluation model.

### User-run smoke and usage follow-up — 2026-10-04

The user supplied a successful live dog/cat report: one request, two candidates,
ten completed Pass checks, and provider-reported usage of 199 input/21 output
tokens. This supersedes the original unverified status above for that synthetic
request only; it does not establish exercise acceptance or change A1 evidence.
See the [smoke record](docs/G1.md#user-run-live-smoke--2026-10-04).

Added an original CC0 topic/past/negative-past fixture and recommended `--release`
in usage examples. Its illustrative sentences were checked offline with the real
pinned analyzer; performance measurements and deferred ideas follow below. These
follow-ups change documentation and synthetic input only. No agent provider call
or credential access was needed.

### Product direction after G2 — reconciled 2026-10-05

The primary outcome is **"WOW! I CAN READ JAPANESE!!"** through meaningful,
comprehensible reading. Prefer reliable offline preparation, generation,
validation and narrowly defined repair; AI is one bounded component.

G2 already delivers focused context selection, optional offline request preview
and one-command experimental generation. The G1.1 interactive REPL plan remains
abandoned; a REPL or daemon/client-server architecture is not the current priority.
The vocabulary-boundary follow-up and managed dictionary loading are implemented.

The remaining planning gap is connecting existing offline learner preparation
to focused generation through an inspectable plan with explicit permissions and
unsupported mappings. Ground reading representation, absent kana-only readings,
reading/sense association and sense-specific direct-object evidence; source
eligibility, glosses, parts of speech and examples cannot establish these mappings.
Start with personalized sentences; coherent short readings remain the destination.
This is a planning recommendation, not implementation approval or a claim of
validated generation. G1/G2 generation, evaluation and acceptance boundaries
remain unchanged.

### Optimization follow-up — 2026-10-05

Original 2026-10-04 local macOS offline analysis: debug **10.2s**, release **0.8s**
(three-run medians; identical reports; compilation/API time excluded). Separate
release measurement: **~256 MB peak process memory**.

- **Now:** use `--release`.
- **Abandoned for now:** G1.1 interactive session. Future long-lived services may
  reuse the analyzer when that architecture is needed for the product.
- **Implemented:** managed dictionary mapping, merged in PR #12. Explicit import
  fully verifies the installation; managed startup checks and file-stability
  obligations follow [the dictionary contract](docs/DICTIONARY.md). Explicit
  external dictionaries still receive full checksum verification into owned bytes.
  The delivery record below contains the Linux release measurements and limits.
- **Implemented:** SHA-256 test-profile optimization, merged in PR #11, with
  Linux/macOS verification recorded above. Ordinary development and release
  profiles are unchanged.
- **Avoid SQL conversion/pruning:** no demonstrated benefit; removing entries
  can change analysis.

Other optimization ideas remain deferred, without implementation approval.
Preserve linguistic pins and A1 history.

## G0 — Manual candidate preview

This delivered slice precedes learner-source composition, Japanese analysis,
and LLM generation. It implements `SPEC.md`'s **Manual candidate preview**
contract and the minimal adaptation described in `ARCHITECTURE.md`.

### Outcome

```sh
yomibu preview \
  --word '猫:ねこ:cat' \
  --word '犬:いぬ:dog' \
  --word '学校:がっこう:school' \
  --grammar 'です' --grammar 'は' \
  --take 2
```

Returns the first two structured word entries and check results. Grammar and
linguistic correctness are explicitly unassessed. This is an executable preview,
not a validated Japanese exercise. The same behavior is callable from Rust with
structured values, without a store or CLI process.

### Implementation sequence and acceptance

Work through these in small confirmed Red-Green-Refactor cycles. Record the
observed RED, GREEN, and explicit refactor review as work completes; do not write
the full implementation before its tests.

1. **Structured input and deterministic selection.** Validate required word
   fields and grammar descriptions, require a positive count within input size,
   and retain input order and associations of text/reading/meaning. Cover empty
   input, invalid counts, duplicates, and the same spelling with different
   reading/meaning entries. Do not infer lexical equivalence or mastery.
2. **Independent result checks and reporting.** Check the produced selection
   against supplied entries and the requested count. Exercise production check
   logic with a valid selection and deliberately invalid data, including an
   unsupplied reading/meaning combination and a wrong count. Preserve grammar
   inputs and report them as unassessed. No tokenizer, model, or fake linguistic
   assessment is involved. Export only types needed by actual library callers.
3. **Thin CLI composition.** Add repeatable `--word 'TEXT:READING:MEANING'`,
   repeatable `--grammar`, and required `--take`. Split at the first two colons,
   reject missing/blank fields, preserve colons inside meanings, and show useful
   errors/help. Render the selected entries and assessment limits. Keep syntax
   parsing outside the library and direct-call behavior equivalent.
4. **Isolation and regression verification.** Run preview with no HOME or token
   in an isolated child-process environment and confirm it creates no data
   directory/cache. Move data-dir resolution into storage-dependent CLI paths;
   do not change global environment in parallel tests. Existing sync/status,
   account isolation, locking, and durability semantics must remain covered.
5. **Review and delivery.** Review naming, ownership, public surface, synchronous
   flow, and whether any trait is justified by actual substitution. No separate
   architecture rewrite, adapter migration, workspace split, or generic plugin
   host. Run the gates below, demonstrate the CLI and direct library use, update
   this section with actual evidence and limitations, and open a reviewable PR.

Definition of done:

- [x] The example selects exactly `猫:ねこ:cat` and `犬:いぬ:dog` in order.
- [x] Required fields, delimiters, counts, duplicates, and meaning/reading
  association have behavioral coverage; invalid inputs never report success.
- [x] Result checks can detect invalid candidate data independently of selection.
- [x] Grammar inputs survive preparation and remain explicitly unassessed.
- [x] Direct library use is synchronous and needs no CLI/runtime/account/store.
- [x] The CLI works without HOME, credentials, cache, or filesystem writes.
- [x] Existing sync/status and persistence regression tests pass.
- [x] `cargo fmt --check`, locked Clippy with all targets/features and warnings
  denied, and `cargo test --locked --all` pass; report any platform limitations.
- [x] The final diff and public API are reviewed; TDD/refactor evidence is
  recorded; documentation reflects implemented behavior; changes are committed
  and available in a pull request. Do not mark this done at planning time.

### G0 implementation TDD record — 2026-10-01

Started from latest merged `main`, `96b3eda`, after confirming the G0 contract.
Each row records a confirmed RED before production code, GREEN, and an explicit
refactor review followed by a focused rerun.

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Structured selection and counts | Integration tests could not import the absent `preview` API | Select a borrowed prefix, preserving duplicates and associated fields; reject zero, excessive counts, and empty input. Three tests passed. Reviewed names, ownership, and test cases; borrowing avoids cloning strings, with no further abstraction justified. |
| Required word fields | A blank text field was accepted | Validate every supplied entry, including the unselected suffix, for blank text/reading/meaning; return an entry-specific typed error. Four tests passed, including Unicode whitespace cases. Reviewed the compact field loop and test matrix; no additional type/helper or refactor was justified. |
| Grammar declarations | The result lacked grammar inputs; after retention was added, a blank description was still accepted | Preserve descriptions, order, duplicates, and optional empty grammar input; reject blank descriptions with a typed error. Six tests passed. Reviewed borrowing and validation: share the input lifetime, retain description content verbatim, and avoid a grammar identity/adapter type; no further refactor justified. |
| Independent checks | Check function/outcome types were absent | Production checks accept supplied entries, reject unsupplied/recombined text-reading-meaning entries, and detect empty/excess selections independently of membership. Public results report grammar and linguistic correctness as unassessed. Three checker tests and six direct API tests passed. Refactored input validation into a private function so validation, selection, and checking are explicit; kept the checker private with no trait or generator seam. Focused suites passed again. |
| Thin CLI and syntax | The example/error cases failed on the unrecognized `preview` command; delimiter tests could not find the parser | Added executable-only parsing/rendering around the library call. Two parser tests cover ASCII delimiters, boundary trimming, internal whitespace, and meaning colons; two CLI tests compare the documented example to direct library use and cover eleven invalid-input cases. Reviewed ownership, error boundaries, and rendering duplication; shared library field validation and one outcome-rendering loop suffice, with no further abstraction justified. Focused tests passed after formatting/review. |
| Preview isolation | Preview failed with “HOME is unavailable” in an environment-cleared child process | Resolve data directories only in sync/status through a shared private helper. Preview succeeds with no HOME/token, an unusable or absent explicit directory, and an invalid token; isolated directories and sentinel bytes remain unchanged. All twelve CLI tests passed. Refactor review preserved sync/status error ordering and kept runtime/source construction in sync; no further change justified. The complete CLI suite passed again. |

### G0 verification and review

Executed on Linux/x86_64 with the repository's exact Rust 1.98.1 toolchain:

- `cargo fmt --check` — passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `cargo test --locked --all` — 84 test entries passed, none ignored: 28 library,
  3 binary, 5 App, 13 cache, 12 CLI, 6 preview, 6 store, 6 summary, and 5 sync.
  This includes the two existing subprocess helpers. The existing rustdoc
  example also compiled successfully.
- Ran the documented command above through `env -i` with no HOME/token. Output:

  ```text
  Manual candidate preview (not a validated Japanese exercise)
    Word: 猫:ねこ:cat
    Word: 犬:いぬ:dog
    Grammar: です
    Grammar: は
  Supplied-entry membership: pass
  Requested entry count: pass
  Grammar: not assessed
  Readings, meanings, naturalness: not assessed
  ```

- `cargo run --locked --example preview` — passed; the synchronous example
  asserts the same selected entries, retained grammar, and check outcomes, then
  prints the structured result. The compiled example also passed under `env -i`.
- Inspected top-level and preview help. `git diff --check` passed. The complete
  diff, including new source/tests/example files, was reviewed for scope,
  ownership, public API, errors, and synchronous data flow.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` — passed, including
  the new public API links.

Final refactor review kept one cohesive preview module and five public data/error
types needed by callers. Input validation and result checking remain private;
no substitution need justifies a new trait. The result borrows immutable inputs,
so words/grammar are not cloned. The CLI alone parses delimiters, reads the
environment for storage commands, renders output, and selects exit status.
Manifest, lockfile, toolchain pin, source/store contracts, persistence code,
schema-1 fixtures, and existing tests are unchanged. Their full regression suite
passed, including account isolation, writer locking, failure preservation, and
uncertain durability.

The Cloud environment initially had no Rust installation. Installed the pinned
toolchain and downloaded the locked dependencies into the workspace; no
repository dependency/toolchain changes were required. Network-enabled execution
was needed for fetching/build setup and the full loopback HTTP test suite.
Preview demonstrations themselves used no network or credentials.

Hosted verification of implementation commit `f77809a` also passed on both
Ubuntu and macOS in [CI run 36921406965](https://github.com/kalorz/yomibu/actions/runs/36921406965).
Both jobs completed formatting, locked Clippy, all tests, and unchanged-lockfile
checks successfully. This is G0 evidence; earlier hosted results elsewhere in
this document remain historical. The follow-up recording this run changes only
this document.

Limitations: local execution was on Linux/x86_64; macOS was verified through
GitHub-hosted CI. No live WaniKani or model call was made. Membership proves only
exact supplied-entry association, not dictionary truth, grammar usage,
naturalness, or Japanese correctness. No later milestone started.

Rust notes for a Ruby developer: `Preview<'a>` borrows slices, so the compiler
prevents changing or dropping the inputs while the result still refers to them.
`Result<_, PreviewError>` and enum outcomes keep input failures and unassessed
checks explicit instead of relying on exceptions or truthy values. An ordinary
synchronous function is sufficient here; no service object, runtime, or new
trait is needed.

### G0 review follow-up — rendering and membership lookup

Addressed the two findings in Greptile's [PR #3 review](https://github.com/kalorz/yomibu/pull/3#issuecomment-5939929148).

- **Rendering RED:** a new subprocess regression failed because embedded newlines
  created extra word/assessment lines and terminal controls were emitted raw.
  **GREEN:** apply `str::escape_debug` when rendering each word field and grammar
  description. The test covers all three word fields, grammar, LF/CR/tab, terminal
  escape sequences, Unicode line/paragraph separators, and literal backslashes.
  The library inputs remain unchanged. **REFACTOR:** reviewed production/test
  code and kept the standard borrowed display iterator, avoiding allocated
  replacement strings, input restrictions, or a custom escaping helper. No
  further refactor was justified; all thirteen CLI tests passed after review.
- **Membership refactor:** the supplied-prefix lookup made N(N+1)/2 comparisons
  for N distinct selected entries. Existing independent checker and public API
  tests passed before changing this behavior-preserving implementation. Replaced
  repeated slice scans with one borrowed `HashSet`; `WordEntry` derives `Hash`
  alongside complete-entry equality. The result still comes from the original
  slice, preserving order and duplicates. No new type, dependency, or adapter was
  introduced. This is a performance refactor, not a new behavioral contract;
  no artificial failing behavior test or timing assertion was added. Reviewed
  ownership, exact-entry matching, hashing cost, and auxiliary memory. All three
  checker tests and six public preview tests passed again after that review.

A temporary direct-library probe measured the median of three calls per size,
with distinct, fixed-width text entries and `take` equal to input size. Input
construction and output were outside the timed calls. The same debug-build probe
was linked against the library before and after the refactor:

| Entries | Slice scans | Borrowed hash set |
| --- | --- | --- |
| 1,000 | 7.590 ms | 1.367 ms |
| 2,000 | 30.242 ms | 2.716 ms |
| 4,000 | 120.492 ms | 5.431 ms |
| 8,000 | 456.895 ms | 11.184 ms |

These are local Linux/x86_64 diagnostic measurements, not release benchmarks,
CI thresholds, or a worst-case timing guarantee. The new check trades O(n)
borrowed-entry storage and string hashing for expected O(n + k) entry operations;
hash collisions still use exact equality. No production input limit was added.

Follow-up verification on Linux/x86_64 with pinned Rust 1.98.1 passed:

- `cargo fmt --check`.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `cargo test --locked --all` — 85 test entries passed, none ignored, plus the
  existing rustdoc example. The only added test is the rendering regression;
  existing source, storage, sync/status, and preview coverage remains intact.
- Reviewed the complete follow-up diff and public API; `git diff --check` passed.
  All three design documents reflect the fixes. Dependencies, toolchain pin,
  schema-1 persistence, and sync/status implementation remain unchanged.

Rust note for a Ruby developer: `HashSet<&WordEntry>` stores references and uses
the entry's derived value hash/equality, without copying its strings. It supports
membership lookup; it does not supply output ordering or remove output duplicates.
`escape_debug()` is a display iterator, so escaping does not modify source values
or require allocating replacement strings.

### Scope limits and next decisions

G0 does not add file/stdin/JSON imports, real Japanese generation or grammar
matching, model calls, provider integrations, storage schema changes, or future
text/plugin types without an executable use. Existing concepts in the target
architecture are guidance, not a checklist of types to implement.

After G0, continue with learner constraints/retrieval and an evidence-backed
Japanese analysis slice, then validated generation. Before claiming linguistic
coverage, select pinned tools/data, review applicable licenses, and evaluate
representative known/unknown and ambiguous cases. Typed model-task composition,
global budgets, basic naturalness/coherence review, and complete post-repair
revalidation arrive with real generation. Broader translation judging and
adaptive selection remain later milestones. No real-model budget is authorized
by G0.

### Original Cloud handoff (planning record)

Merge the documentation-only planning PR before starting from `main`. Otherwise,
explicitly select its branch, `codex/generation-preview-plan`, as the starting
point. A local saved file or commit alone is not the remote handoff. Confirm this
G0 section exists in the selected checkout before implementation; do not recreate
decisions from the old shared-chat transcript. The three repository documents
contain the consolidated direction and are sufficient task context.

Suggested implementation prompt:

```text
Work in kalorz/yomibu from the latest main containing "G0 — Manual candidate
preview" in PLAN.md. Read AGENTS.md, SPEC.md, ARCHITECTURE.md, and PLAN.md, then
inspect the actual implementation before editing. If the G0 plan is absent,
report the base-branch mismatch rather than inventing the missing requirements.

Implement only G0 end to end on a new codex/ branch: a synchronous public library
preview and the thin CLI command described in SPEC.md. Preserve structured
text/reading/meaning associations, grammar declarations, deterministic selection,
independent membership/count checks, typed errors, and honest unassessed statuses.
Preview must work without HOME, tokens, a store, network access, or an async
runtime. Keep existing sync/status behavior and schema-1 persistence intact.

Use strict Red-Green-Refactor, confirming each focused RED before production
behavior and explicitly reviewing/refactoring after GREEN. Keep the public API
small and adapt existing code incrementally; do not scaffold the future text
hierarchy, language analysis, model integration, or a general plugin framework.
Proceed autonomously on routine reversible choices; do not stop after a plan.

Run cargo fmt --check, cargo clippy --locked --all-targets --all-features -- -D
warnings, and cargo test --locked --all. Demonstrate the example command and
direct library use. Update PLAN.md with actual TDD/refactor evidence, check
results, and limitations; keep the specification and architecture aligned.
Review the complete diff, commit and push the implementation, and open a PR.
Report the PR link, commit, verification, and brief Rust notes for a Ruby
developer. Do not merge automatically or start the following milestone.
```

### Planning verification — 2026-10-01

Prepared from merged `main` at `b2b50e8`. Reviewed the three-document diff for
consistent scope, vocabulary, CLI contracts, and acceptance criteria; checked
whitespace with `git diff --check`. Only SPEC.md, ARCHITECTURE.md, and PLAN.md are
changed. No Rust source, tests, dependencies, or CI configuration are changed,
and no Cargo gates or linguistic/model experiments were run for this
documentation-only handoff. At that point G0 remained unimplemented; its delivery
and verification are now recorded above.

## Learner constraints and retrieval — approved preparation slice

Approved on 2026-10-02: implement only offline practice-context preparation from
preserved WaniKani progress, local manual grammar assertions, a revisable concrete
knowledge policy, and explicit word/reading/sense targets. SPEC.md defines the
behavior and ARCHITECTURE.md the composition boundaries.

The default uses a recorded lesson start; recorded pass is an explicit alternative.
Both exclude unavailable/hidden material and require an assignment. Grammar
descriptions assert familiarity; technical IDs are local to the loaded input.
Lexical retrieval uses only cached WaniKani data, with exact accepted fields and
explicit unassessed reading/sense association and example suitability.

Implementation starts from main `22b7c92` on
`codex/learner-context-preparation`. PR #3 and both Greptile fixes were verified
in main. The authoritative G0 documents agree with the code; README omitted G0 and
is now updated with both runnable slices.

During the original implementation the cloud executor failed provisioning. No
local work was inspected or changed in that session. Implementation used GitHub
commits on the separate branch and the existing Ubuntu/macOS PR CI for observed
Red–Green–Refactor execution. Local execution became available in the review
follow-up below; no private learner cache or real-data demonstration is claimed.

Acceptance:

- [x] Grammar input preserves assertions/duplicates/identity and rejects invalid input.
- [x] Policy decisions are explainable and recomputable without mutating source data.
- [x] Targets preserve complete tuples and cannot override learner eligibility.
- [x] Retrieval has explicit missing/ambiguous/unsupported outcomes and provenance.
- [x] CLI/direct calls agree; no implicit sync, runtime, credentials, or writes.
- [x] Runnable synthetic demonstrations and assessment limits are documented.
- [x] Existing G0, sync/status, schema-1 and durability regressions pass.
- [x] Required locked quality gates pass on the final implementation.
- [x] Full diff/public API review, commit/push, and PR delivery are complete.

Develop one behavior at a time: observed RED, minimal GREEN, explicit refactor
review, and focused/full reruns. Record actual evidence below; do not write
production behavior before its failing test. At this implementation stage the
real-learner demonstration was unverified; the 2026-10-03 acceptance record below
supersedes that status.

### Implementation evidence

Initial grammar cycles are recorded in commit history. Confirmed REDs: missing
`grammar` module (`db700412`, run 36994881967), blank assertion accepted
(`99291d6`, run 36995270179), and missing file adapter (`4896ebb`, run 36995571545).
Each received a minimal GREEN, followed by explicit ownership/error/API review.
The preservation test passed again in the blank-validation RED. The file-input
refactor removed an unnecessary dummy read; Ubuntu CI rerun 36996003851 passed.
Knowledge derivation RED was confirmed at `9d3ed01` (run 36996345316), then both
platforms passed at `337a0f7` (run 36996845899). The explicit refactor consolidated
timestamp decisions; its Ubuntu rerun 36997118549 passed. Retrieval RED was
confirmed at `0ef5ca6` (run 36997244224); both platforms passed at `1377265`
(run 36997692088). Retrieval refactor review covered borrowing, indexing, exact
fields, ambiguity, errors and public surface; no further change was justified.
CLI RED was confirmed at `c0e5873` (run 36998635952): new subprocess tests
failed on the unrecognized `prepare` command while domain suites passed. Both
platforms passed at `bb0f7b5` (run 36998983706). Refactor review covered parsing,
error/output boundaries, synchronous composition, and escaping; no change was
justified. Those five CLI cases passed again in the explanation RED below.

The explanation test failed at `a613c84` (run 36999388465) because the report
omitted per-subject decisions. The minimal renderer now reports each identity,
kind, and policy inclusion or typed exclusion. Both platforms passed at
`f9b6096` ([run 36999513134](https://github.com/kalorz/yomibu/actions/runs/36999513134)):
formatting, locked Clippy, all 103 test entries (including two existing subprocess
helpers), one rustdoc example, and unchanged-lockfile checks. The explicit
refactor review kept one loop over the already-derived decisions; it adds no
second interpretation of policy. The final CI run below passed after that review.
Formatting failures during these cycles were corrected separately and are not
counted as behavioral REDs.

Documentation and example/CI infrastructure require no artificial RED. The
workflow now runs both documented CLI demonstrations and direct-library examples,
builds API documentation with warnings denied, checks diff whitespace, and
retains the required locked gates and unchanged-lockfile check on Ubuntu/macOS.
No live learner/model service or credentials are needed.

Refactor review of the whole slice covered naming, modelling, public surface,
error boundaries, duplication, ownership, and idiomatic Rust. It keeps cohesive
modules, borrowed source evidence, concrete policy alternatives, and typed
errors. No further change was justified. No new trait has a demonstrated
substitution need: existing `LearningStore::load` supplies the file or memory
boundary and direct values need neither. G0's implementation, source and store
contracts, persistence/schema, dependencies, and toolchain remain unchanged.
Rust lifetimes make these borrowed views safe without cloning the source graph;
enums keep ineligible and unassessed states explicit.

[PR #4](https://github.com/kalorz/yomibu/pull/4) contains the branch and observed
TDD history. The complete diff was reviewed, including all new source, tests,
fixtures, examples, documentation, and CI changes. No placeholder implementation
or unintended source/persistence/G0 changes remain.

### Final slice verification

Commit `8fdb850` passed [CI run 37032161587](https://github.com/kalorz/yomibu/actions/runs/37032161587)
on GitHub-hosted Ubuntu/x86_64 and macOS/arm64 using the pinned Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all`: 103 test entries passed, none ignored, plus one
  compiled rustdoc example.
- Both documented CLI commands and `cargo run --locked --example preview` /
  `cargo run --locked --example prepare` passed. Preparation selected subject 2,
  assignment 102, and its attached example; decisions were 2 eligible and 3
  excluded. All four linguistic assessment limits remained unassessed.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` passed.
- `git diff --check` against the PR base and the unchanged-lockfile check passed.

The final follow-up records this evidence and wraps documentation only. Hosted
CI also checks each subsequent PR commit. No local test execution, live WaniKani
request, private learner data, model call, or linguistic validation was performed.
At that stage the real-learner demonstration was unverified: automated fixtures
and demonstrations were synthetic. The 2026-10-03 acceptance record below
supersedes that status. No linguistic guarantee is claimed.

### PR #4 review follow-up — 2026-10-02

Read all Greptile summary reviews, submissions, inline comments, and threads.
The one finding, [excluded subjects lack evidence](https://github.com/kalorz/yomibu/pull/4#discussion_r4167630466),
is valid against SPEC's explainability requirement: the library retained the
evidence, but the CLI showed only identity, kind, and exclusion reason.

- **RED:** added `report_attaches_retained_evidence_to_each_policy_decision`.
  The focused test failed because the hidden subject's report omitted
  `Content: available; hidden_at: Some(2026-09-27T09:00:00Z)`.
- **GREEN:** render retained content availability/hidden timestamp, assignment
  ID/hidden flag/start/pass timestamps, and review-statistic ID/hidden flag
  beneath every decision. Explicit absences do not become fabricated records.
  The regression passed across each hidden source, both missing lifecycle dates,
  missing assignments/statistics, and unavailable content with retained progress.
  It also checks unchanged cache bytes and no newly created files.
- **REFACTOR:** reviewed naming, duplication, modelling, borrowing, and tests.
  Three direct `Option` matches render borrowed evidence without cloning or
  interpreting policy again. The existing decision loop and isolated subprocess
  fixture suffice; no new helper, trait, or further refactor was justified.
  All seven preparation CLI tests passed again after this review.

The resumed workspace provides shell/filesystem access and cached Rust 1.98.1.
The original `/workspace/yomibu` checkout was clean and left untouched. Work uses
an isolated checkout on the existing PR branch. Initially, sandboxed Git could
not reach the proxy, so the GitHub connector supplied the exact head and tree,
both verified by Git hashes. With shell network permission, a normal fetch then
restored full history and confirmed main `22b7c92` and PR head `322bc80` had not
advanced. No reset or discarded work was needed.

Local Linux/x86_64 verification passed:

- `cargo fmt --check` and
  `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `cargo test --locked --all`: 104 test entries, including the two existing
  subprocess helpers, plus one compiled rustdoc example; none ignored. The first
  sandboxed attempt failed on denied loopback binds; the rerun with network
  permission passed using only local mock servers and synthetic credentials.
- Documented preview CLI and both direct-library examples; preparation CLI
  with both lesson-started and recorded-pass policies; synthetic status matched
  its golden output. Preparation left cache and grammar bytes unchanged.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`.
- Complete PR/follow-up diff and public API review, `git diff --check`, and
  unchanged dependency/lockfile/toolchain checks. G0 and both earlier Greptile
  fixes, sync/status, source/store contracts, and schema-1 persistence are intact.

SPEC, ARCHITECTURE, and README now describe evidence beside each decision.
There are no deferred findings from this review. Hosted verification of this
follow-up is reported on the PR rather than inferred from earlier green runs.
No live learner/API/model or linguistic validation was performed. Synthetic
results alone did not fulfill real-learner acceptance; that was completed later
as recorded below.

Rust note for a Ruby developer: matching `Option<&T>` distinguishes an absent
record from a present record whose optional timestamp is missing, while borrowing
lets the CLI display the original facts without copying or persisting them.

### Real-learner preparation acceptance — 2026-10-03

Implementation **and real-learner acceptance are complete** at
`493191349228bd0de149280d131f1c388ea7921c`. This record carries forward the user's
completed acceptance evidence; it is not a new execution or inspection of private
data during protocol preparation. Production schema-1 loading and CLI/library
preparation agreed under both policies. Ineligible targets failed without partial
output. Grammar declarations and attached source examples retained exact content,
order, identities, and provenance; inputs and inventories were unchanged.

The user reports prior passing formatting, strict locked Clippy, all 104 test
entries plus rustdoc, demonstrations, and API documentation at this head. These
checks were not repeated for unchanged implementation. Private counts, learner
identifiers, declarations, and detailed reports remain outside Git and external
services. Expiry of temporary evidence does not invalidate completed acceptance.
No new sync or credential inspection is authorized. Eligibility is not mastery;
linguistic validity remains unassessed.

## A1 — review preparation and implementation, 2026-10-03

### Historical protocol-only checkpoint

The following checkpoint preceded explicit implementation approval. The current
implementation and acceptance status follow it below.

Accepted scope: supplied modern Japanese sentences of at most 100 Unicode
characters; vocabulary identity; regular godan/ichidan polite present/past/negative;
narrow topic は and object を; nominal です. Preserve whole compounds and their
components. Other grammar, multiword expressions, and reading/sense ambiguity
expose limitations. No generation, repair, quiz, provider integration, general
grammar/contextual-sense validator, or speculative framework.

The accepted set plan is 12 vocabulary, 12 inflection, 12 particle, and 12 grammar
cases, plus 6 compound/MWE and 6 reading/sense challenges. Core partitions contain
24 development and 24 held-out cases, each with 12 positives and 12 negatives.
Related cases stay in one partition; freeze holdout before tuning and keep it
private until scoring. Held-out targets apply together: zero false acceptance,
at most one false rejection, at most two inconclusive core cases, at least 10/12
correctly passed positives and 10/12 correctly failed negatives, no execution
errors or missing required checks. Challenges must receive no unsupported
acceptance. An honest no-go can complete the investigation.

The user explicitly approved manual blind review by another AI provider in place
of qualified human reviewers. The initial [protocol](docs/A1_REFERENCE_PROTOCOL.md)
separated verified source coverage, provisional model judgments, analyzer results,
and run integrity. Its proposed source gate required all core reference labels
to be source-backed before freezing; model-only/disputed cases cannot establish
the targets. At that checkpoint the gate and detailed operational bindings awaited
agreement, not renewed approval of the provider-review choice.

Prepared a [blank packet](docs/A1_REVIEW_PACKET.md) and
[copyable prompt](docs/A1_EXTERNAL_REVIEW_PROMPT.md) for the user to operate.
Public primary reference pages were opened and citation locations checked;
source limitations and a table discrepancy are retained in the protocol. No
case-specific source coverage or analyzer performance is claimed. No evaluation
sentences, practice passages, model calls, installations, dictionary/corpus
downloads, or implementation occurred. Private learner evidence was not opened.

Git was clean at the start. Fetched origin and confirmed `main`/`origin/main`
remain at `4931913`, with both existing branches and the single worktree preserved.
That checkpoint changed documentation only. Review included the new untracked
documents; Cargo gates are intentionally not repeated for unchanged Rust code.
Documentation verification passed: `git diff --check`, whitespace/final-newline
checks across all seven changed/new Markdown files, all 32 local link targets,
and balanced code fences in the three new documents. Content review checked
scope, evidence tiers, fixed denominators, disagreement retention, holdout
custody, and privacy. Those documentation changes were retained into the implementation branch.

The planned next step at that checkpoint was to verify the recommended, unexecuted
Sudachi.rs v0.6.11 / SudachiDict Core 20260723 V0 pair, pin exact revision,
checksum and configuration, and retain C-mode whole units and A-mode components
with original spans. Use strict small Red–Green–Refactor cycles, deterministic
synchronous library logic, a concrete adapter with real boundary tests, and a
thin evaluation executable/example. Preserve G0, preparation, sync/status and
schema 1; explicit evaluation bindings do not interpret free-form declarations.
Run the required locked gates after changes and align all four main documents
with actual evidence. No general analysis CLI is required.

### Authorized implementation follow-up

The user explicitly approved starting A1 implementation. Inspected Git and fetched
origin before code changes: main/origin/main remained at `4931913`. Created
`codex/a1-offline-analysis` while preserving the protocol edits, both previous
branches, and the single worktree. No learner cache, credential values, or private
preparation evidence was inspected, and no sync or model call occurred.

Implemented `analysis.rs`, the concrete Sudachi adapter with embedded config,
`evaluation.rs`, and `examples/a1.rs`. Exact Git/dictionary/configuration pins,
source/license checks, operational limits, and the cycle-by-cycle Red–Green–Refactor
record are in [A1 implementation](docs/A1_IMPLEMENTATION.md). The public dictionary
was explicitly downloaded only after approval. The adapter owns checksum-verified
bytes, preserves C whole/A component identities and original UTF-8 spans, and has
no ambient configuration, automatic download, or substituted test backend.

The five required checks separate completed judgments from errors and NotRun.
Seven explicit rule variants bind to unchanged manual declarations. Regular verb
class/stem checks, nominal です, narrow topic/object constructions, tuple ambiguity,
compound permissions, and unsupported coverage are exercised by real-adapter
contract tests. Naturalness, MWE, and contextual readings/senses remain unassessed;
a literal-looking idiom can receive a structural Pass. Unsupported challenge
passes still block a protocol go decision. No generation or exercise acceptance
is implemented.

The thin example consumes a bounded synthetic packet and reports all inputs,
analysis/provenance, judgments, limitations, and execution errors. Required real
adapter tests never silently skip when the dictionary is missing. Explicit setup
and retained license notices live under ignored `target/a1`; CI now performs
setup before tests and runs the smoke example without publishing its detailed
report. Cargo.lock adds 25 packages and preserves every existing resolution.

An original reusable draft contains 24 visible development candidates (12 proposed
positive/12 negative) and 12 challenges, with opaque IDs and fixed presentation
order. The first-pass labels/families/alternatives remain in a private provisional
record outside Git. At implementation delivery all reference outcomes were
model-only; no evaluation run
or tuning against this draft has occurred. This exposed material is not blind
holdout. The [review prompt](docs/A1_EXTERNAL_REVIEW_PROMPT.md),
[common binding sheet](docs/A1_BLIND_PACKET_HEADER.md), and
[private custodian handoff](docs/A1_HOLDOUT_CUSTODIAN_PROMPT.md) make the next manual
steps concrete. At implementation delivery the source gate was 0/48 fully supported core outcomes;
held-out targets/challenge safeguards are not evaluated. Investigation acceptance
remains pending, rather than being inferred from engineering tests.

Verification executed on macOS/arm64 with pinned Rust/Cargo 1.98.1:

- `cargo fmt --check` passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` passed after
  boxing the large upstream error cause, preserving typed source chains.
- `cargo test --locked --all` passed: **122 test entries plus one rustdoc = 123**,
  zero failures/ignored. The initial sandbox attempt could not bind local mock
  ports; rerunning with loopback permission passed, without live services.
- Real adapter tests passed again after adding same-size wrong-checksum coverage;
  the child-process ambient-configuration isolation probe also passed.
- API documentation built with `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`.
- The A1 smoke executable produced Pass/Fail/Inconclusive with zero execution
  errors and all five checks. A missing dictionary produced three errors/NotRun
  and nonzero exit status. Invalid setup archives preserved the existing dictionary.
- Existing preview and prepare CLI/library demonstrations passed using repository
  synthetic fixtures only. No real-learner acceptance was repeated.
- Final formatting/strict locked Clippy passed after the adapter checksum probe.
  Documentation audit passed across 12 changed/new Markdown files (newlines,
  whitespace, fences, and all 59 local links). `git diff --check` passed; the
  dictionary remains ignored. The six-case first review packet exactly matches
  the first six draft inputs, with no reference labels or analyzer output.
- Review draft structure was checked without analyzer execution: 24 development
  candidates, 12 challenges, opaque unique IDs, explicit bindings, scalar limits,
  retained provisional metadata, and exact packet hashes. No linguistic/source
  validity or blind score is inferred from these structural checks.

No Linux/container or hosted CI run is claimed for this A1 change. Existing
preparation acceptance was not repeated; its completed evidence remains valid.
No commit, push, or publication is implied by local implementation. The full
linguistic investigation needs source reconciliation and private holdout custody.

Rust notes for a Ruby developer: `Sentence<'a>` borrows exact source text, enum
states prevent boolean ambiguity, and a concrete owned analyzer supplies reusable
synchronous operations. `Box` keeps the large upstream error off the success-path
Result while retaining its original cause. No service-object framework or async
layer was added.

### First returned blind review and source audit — 2026-10-03

Preserved the exact six-case external reply, reviewed v0.2 packet, initial private
judgments, and their SHA-256 hashes outside Git. The provider/model/settings were
not supplied and remain unknown. Reopened decisive TUFS grammar passages and
checked actual named Shogakukan dictionary entries on Kotobank. Jisho access
failed; the reviewer's JMdict entry IDs remain unverified, even where replacement
sources support the same lexical fact. Search snippets did not establish evidence.

Reconciled all six cases provisionally: **3/48 core reference outcomes and 3/12
challenge references** are now source-backed. This is reference coverage, with
no analyzer score. Preserved alternative meanings and review overclaims; changed
two grammar check judgments where nominal/adjectival applicability was unknown.
Overall recommendations did not change. Protocol v0.3 clarifies lexical attestation
versus contextual sense selection within the existing bounded scope. Thresholds,
fixtures, bindings, runtime code, and the original v0.2 packet are unchanged.

Prepared the next six existing cases in a complete v0.3 blind packet. Detailed
reviews, source locators, differences, and provisional adjudications remain private.
No new sentences, model calls, analyzer benchmark runs, learner reads, sync,
installation, or dictionary/corpus downloads occurred. Holdout remains unseen and
unfrozen here. This reference/documentation-only follow-up does not repeat Cargo
gates; the implementation verification above remains the latest executed evidence.
Documentation checks passed for 13 Markdown files and 60 local links; code
fences, final newlines, whitespace, and `git diff --check` passed. All six reported
scalar/byte inventories and source-reference links in the private ledger were
checked mechanically. Hashes confirm the original reply, first packet, initial
reference record, and review draft remain unchanged. The second packet is exactly
cases 7–12 with the v0.3 prompt/header, without reference labels or analyzer output.

### Second returned blind review and source audit — 2026-10-03

Preserved the exact reply and reviewed v0.3 packet, initial judgments, preceding
audit, and hashes outside Git. Newly inspected the cited dictionary entries and
relevant polite-form grammar; reused unchanged previously inspected grammar and
lexical evidence with explicit provenance. Model self-identification is recorded
as self-reported, without inferring the actual service/model version.

All six overall recommendations were retained provisionally. Two particle
checks changed from Pass to Inconclusive: an unresolved expression cannot inherit
scoped object applicability solely from the component verb's transitivity.
Dictionary expression headwords attest alternatives, not one morphological token
or an automatic missing-word permission violation. These are applications of
existing v0.3 limits; no scope, threshold, fixture, binding, or runtime change.
Detailed corrections and alternatives remain in the private claim ledger.

Cumulative reference coverage is **7/48 core outcomes and 5/12 challenge
references**, from 12 visible cases. Another 24 visible cases await review;
the separate 24-case holdout remains unseen here. Prepared exactly the next six
existing cases as the third blind packet, with the same v0.3 prompt and bindings.
No benchmark run, tuning, learner access, sync, model call, installation, or
dictionary/corpus download occurred. Cargo gates were not repeated for this
reference/documentation-only follow-up; previous implementation evidence stands.
Validation passed: 14 Markdown files, 61 local links, fences/newlines/whitespace,
and `git diff --check`. Private hashes, source-ID references, and scalar/byte spans
were checked mechanically. Both reviewed packets and the full draft are unchanged;
the third packet contains exactly cases 13–18 with no answers or analyzer output.

### Third returned blind review and source audit — 2026-10-03

Preserved the unedited user-message reply, reviewed v0.3 packet, original
judgments, preceding audits, and hashes outside Git. Freshly inspected the new
dictionary entries, the publisher's noun-label convention, and the topic-less
nominal construction; reused earlier source inspections with explicit provenance.
Provider/model/settings remain unknown rather than inferred.

All six overall and thirty check recommendations were retained provisionally.
Empty grammar permissions authorize no rules; they do not disable required
checks. The reviewer's alternative interpretation remains in the private ledger.
Claims of exhaustive uniqueness and naturalness under every reading were narrowed
to supported lexical facts and bounded rule application. Unverified JMdict claims
remain candidates only. No scope, threshold, fixture, binding, or runtime change.

Cumulative reference coverage is **11/48 core outcomes and 7/12 challenge
references**, from 18 visible cases. Another 18 visible cases await review;
the separate 24-case holdout remains unseen here. Prepared exactly the next six
existing cases as the fourth blind packet with the same v0.3 prompt and bindings.
No benchmark run, tuning, learner access, sync, provider call, installation, or
dictionary/corpus download occurred. Cargo gates were not repeated for this
reference/documentation-only follow-up; previous implementation evidence stands.

Validation passed: 15 Markdown files, 60 local file links, fences, final newlines,
whitespace, and `git diff --check`. Private input hashes, source references, all
five check inventories, and original scalar/byte spans were checked mechanically.
All three reviewed packets and the full draft remain unchanged. The fourth packet
contains exactly cases 19–24 with the unchanged v0.3 prompt/header, no answers,
and no analyzer output.

### Fourth returned blind review and source audit — 2026-10-03

Preserved exact attachment bytes, the reviewed v0.3 packet, original judgments,
previous audits, and hashes outside Git. Opened the new decisive lexical passages;
reused prior grammar and lexical checks with explicit provenance. Provider/model
metadata and unopened JMdict candidates remain unknown/unverified.

Retained all six overall recommendations provisionally. Corrected two check
judgments to Inconclusive because the cited dictionary also records adjectival
use with historical examples; modern applicability is unresolved. An established
vocabulary violation still yields Fail under the existing contract. Corrected
one original-span/dictionary-base mismatch. A dictionary expression entry does
not establish one morphological word or an automatic permission violation.
All disagreements and source restrictions remain in the private claim ledger.

Cumulative reference coverage is **15/48 core outcomes and 9/12 challenge
references**, from 24 visible cases. Twelve visible cases and the separate unseen
24-case holdout remain. The fifth packet contains exactly the next six existing
cases with the same v0.3 prompt and bindings. Replaced stale duplicated progress
in the protocol with a link to the current evidence status; no contract change.
No benchmark, tuning, learner access, sync, provider call, installation, or
dictionary/corpus download occurred. Cargo gates were not repeated for this
reference/documentation-only work; previous implementation evidence stands.

Validation passed: 16 Markdown files, 62 local file links, fences, final newlines,
whitespace, and `git diff --check`. Private input hashes, source references,
five-check inventories, and original scalar/byte spans were checked mechanically.
The four reviewed packets and full draft remain unchanged; the fifth packet is
exactly cases 25–30 with the unchanged v0.3 prompt/header and no answer key.

### Fifth returned blind review and source audit — 2026-10-03

Preserved the unedited user-message reply, reviewed v0.3 packet, original
judgments, prior audits, and hashes outside Git. Inspected new lexical passages,
component-formation evidence, and the applicable topic/object verb pattern;
reused earlier source checks with explicit provenance. The reviewer's reported
derived JMdict build, metadata, and IDs remain unverified. Publisher evidence
supports adopted claims without a coordinator dictionary download.

Withheld two proposed core positives whose reading alternatives were dismissed
without adequate evidence; one also retains noun/adjective uncertainty. Four
check judgments changed to Inconclusive. Their reference outcomes remain
unresolved and cannot count toward the 48-case gate. These are not analyzer
Inconclusive results. Preserved all initial labels and families; no replacement,
rebalancing, or denominator reduction occurred. A challenge's initial provisional
recommendation changed to Fail after additional source evidence explicitly
established word formation and named the compound. The original decomposition
alternative and rejected reasoning remain in the private record.

Cumulative coverage is **18/48 core outcomes and 10/12 challenge references**,
with **two reviewed core references unresolved**, from 30 visible cases. Six
visible cases and the separate unseen 24-case holdout remain. Prepared exactly
the final six existing visible cases with the unchanged v0.3 prompt and bindings.
Replaced another stale duplicated progress statement with a status link. No
contract, threshold, fixture, binding, or runtime change; no benchmark, tuning,
learner access, sync, provider call, installation, or coordinator dictionary/corpus
download. Unchanged Cargo gates were not repeated for reference/documentation work.

Validation passed: 17 Markdown files, 64 local file links, fences, final newlines,
whitespace, and `git diff --check`. Private hashes, all five check inventories,
source references, and original scalar/byte spans were checked mechanically.
All five reviewed packets, the full draft, initial proposals and family assignments
remain unchanged. The sixth packet is exactly cases 31–36 with the unchanged
v0.3 prompt/header. Validation separately counts supported and unresolved references;
neither unresolved core case receives a binary reference outcome or gate credit.

### Sixth returned blind review and source audit — 2026-10-03

Preserved the supplied attachment byte for byte, reviewed packet, initial
proposals, prior audits and hashes outside Git. Checked the new lexical passages
and broader particle source; reused earlier inspections with explicit provenance.
All six overall recommendations remain supported provisionally. One vocabulary
check becomes Inconclusive consistently with earlier reading uncertainty; the
independently supported topic-permission violation survives it. Source text for
the full place-name and its abbreviation supports the whole-identity finding;
component decomposition does not grant a composition permission. Phrase entries
still do not automatically establish whole words. Reviewer self-identification
remains attributed, without service UI confirmation.

Exact binding-sheet review exposed nine earlier absent-feature Pass findings
inside unresolved constructions. Recorded a separate append-only correction to
Inconclusive; preserved every original audit and reply. Overall outcomes and
reference-gate eligibility are unchanged. This corrects application of the
existing construction requirement, not the contract or analyzer behavior.

All **36/36 visible cases** have now been reviewed: **22/48 supported provisional
core outcomes, 12/12 challenge references, two unresolved core positives**.
The separate 24-case holdout remains unseen; no performance result follows.
Prepared one focused unblinded clarification packet containing only the two
unchanged unresolved synthetic cases and source questions, outside Git. This is
the single clarification allowed by protocol step 5, not another blind batch.
If evidence remains insufficient, preserve it and use documented pre-freeze
family replacement or an evidence-limited no-go; never seek a model majority.

No sentence, fixture, binding, threshold, or runtime change; no benchmark,
tuning, learner access, sync, provider call, installation, or dictionary/corpus
download. Unchanged Cargo gates were not repeated for reference/documentation
work. Validation checks all six packet snapshots, original labels/families,
private hashes, five-check inventories, source references, UTF-8 spans, the
append-only correction overlay, and exact two-case clarification payload;
Markdown links/fences/whitespace and `git diff --check` also pass.

### Focused clarification and pre-freeze draft revision — 2026-10-03

Preserved the exact returned clarification, reviewed packet, earlier audits and
hashes outside Git. Reopened the decisive dictionary passages; both proposed
positives remain unresolved, with no agreement-based promotion. Corrected the
reviewer's assertion of an explicit POS label absent from the displayed entry
and the reason for a no-nominal-occurrence check. The model's claimed interface
identity/settings remain self-reported. The single clarification is finished;
do not repeat these cases to seek a different answer.

Used the existing protocol's pre-freeze replacement provision. Preserved the
original draft and every reviewed packet byte for byte; authored revision 2 with
four replacement inputs in the same two family lineages, including their paired
negatives. Retained old/new mappings, exclusion reasons, exact inputs, alternatives
and initial source-inspected proposals privately. New cases have opaque IDs and
CC0 provenance, and await a fresh four-case blind review with unchanged v0.3
prompt/bindings. No source exercise or learner material was copied. No analyzer
run informed selection. Both versions remain excluded from private holdout.

Active coverage is **20/48 supported provisional core outcomes and 12/12 challenge
references**, with four new references awaiting blind review and 24 holdout cases
unseen. The original 22 supported outcomes and two unresolved positives remain
historical evidence; two supported negatives leave the active set with their
families. Counts are not added across revisions. Category balance, family rules,
planned 60 cases and thresholds remain unchanged.

Inspected Git state and fetched origin before preparing the revision; main and
origin/main remain at `493191349228bd0de149280d131f1c388ea7921c`. No Rust/runtime
behavior change, installation, dictionary/corpus download, provider call, learner
access, sync, benchmark or tuning. Cargo gates are unchanged and not repeated for
reference/fixture documentation. Validation passed for preservation, counts and
family balance, new JSON bindings/UTF-8 spans, exact blind payload and prompt,
source IDs, 19 Markdown files with 74 local links, and `git diff --check`.

### A1 revision-2 four-case review audit — 2026-10-03

Preserved the complete returned four-case review, exact reviewed packet/prompt,
initial proposals, source provenance and earlier history outside Git. Retained
all four overall and twenty check recommendations after citation reconciliation.
Existing v0.3 grammar permissions and the explicit topic slot settle the two
contract questions; discourse roles remain unassessed. Reviewer alternatives and
stronger assertions remain recorded, with lexical uniqueness/component claims
narrowed. Reopened publisher receiver/noun definitions, POS convention and the
TUFS topic/object frame; reused earlier actual lexical/morphology inspections
with explicit provenance. The reported redistributed JMdict file, entry IDs and
snippet remain unverified and are not decisive evidence. Model metadata remains
attributed rather than independently confirmed from the service UI.

Active reference coverage is now **24/48 core outcomes and 12/12 challenge
references**, all source-backed and provisionally adjudicated. Development keeps
12 positives and 12 negatives, three of each per category. A new private aggregate
ledger applies prior check corrections without modifying the original audits.
Retired families remain excluded, not relabeled or counted twice. The full gate
still lacks 24 private holdout references; nothing is frozen or scored.

Prepared one combined local custodian handoff from public process documents,
both synthetic draft versions, smoke inputs and an exposure inventory of public
test literals/generated-form families. It contains no private answers or learner
data. The user supplies it to a separate non-implementation conversation; hidden
inputs/reviews stay there until scoring. No new evaluation sentences were authored
in this audit. No code changes, installs, downloads, provider calls, learner access,
sync, benchmark or tuning occurred; unchanged Cargo gates were not repeated.

Validation passed for 198 preserved file hashes, all 180 active reference-check
findings and original UTF-8 spans, exact reviewed packet/prompt, source namespaces,
category/family balance, 18 Markdown files with 75 local links, and the public-only
handoff assembly. These are integrity/source-coverage checks, not analyzer results.

### A1 reference freeze, visible evaluation and implementation freeze — 2026-10-04

Accepted the non-revealing held-out freeze receipt as the separate custodian's
attestation: 24 reviewed/reconciled source-backed provisional references, with
five exposed families replaced and all historical evidence retained. The 24
visible core and 12 challenge references were already audited. Froze their exact
inputs, reference key, source/history manifests and prompt/binding records before
running any evaluation. Hidden reference files remain outside this chat; provider
metadata gaps stay recorded, without inferring independent linguistic truth.

Inspected Git and fetched origin: local HEAD and origin/main remain
`493191349228bd0de149280d131f1c388ea7921c`; all work and branches are preserved.
Saved the dirty workspace's build inputs and source archive explicitly. The first
36-case visible run matched every development outcome and all 120 core check
judgments, but four negative inflection findings had broader-than-reference spans.

Strict TDD reporting fix: strengthened the existing real-adapter four-form test;
RED failed on `0..12` versus `6..12`. Changed only the finding's start from the
verb's start to its end, retaining exact original bytes. GREEN passed. Reviewed
production/test naming, duplication, borrowing and modelling: the existing
recognizer guarantees the stem/auxiliary boundary, so no helper, allocation or
new type was justified. The explicit post-refactor-review rerun passed.

The second visible run keeps all outcome/check judgments and now matches all
12 negative reasons and decisive spans. Challenges produced 1 Fail, 8 Inconclusive
and 3 unsupported Pass results, with 38/60 reference check judgments matching.
The three unsupported passes fail the safeguard; the split-compound and individual
check mismatches remain recorded rather than hidden by matching overall labels.
Both runs had zero execution errors/missing checks and preserved original input
and C/A spans. Raw outputs were hashed before private reference comparison.
No reference or challenge-specific behavior was changed to improve the tally.

Post-change formatting and strict locked all-targets/all-features Clippy passed.
Locked tests initially failed because the sandbox denied local mock-server ports;
the permitted retry passed all 122 test entries plus one rustdoc. Dependency use
was offline. No real learner access, credentials, sync, model call or download.

Declared implementation/configuration freeze `a1-implementation-2026-10-04-v1`
after the fix and gates. Recorded 58 exact build-file hashes, dirty-base commit,
tracked diff, source archive, frozen executable, toolchain, upstream revision,
dictionary/configuration pins and reference freeze. The single 24-case held-out
run remains pending exact input release; no held-out input/answer has been opened.
The current challenge failure already prevents a bounded go. The held-out run
will complete core evidence under the original thresholds, with no after-score
repair in that claimed run. Detailed receipts, logs and reports remain private.

### A1 single held-out execution — 2026-10-04

Verified the user-released input against the recorded freeze hash and verified
the saved executable, 58 build files, dictionary, configuration and reference
freeze. Ran the existing executable once offline, without a rebuild or change.
Preserved complete stdout/stderr and their hashes before report inspection.
Integrity checks confirm 24 exact inputs in order, 120 completed required checks,
zero execution errors/NotRun cases, original UTF-8/C/A spans, and pinned provenance.
No correctness comparison or private answer/source-ledger access occurred here.
Raw output and a handoff receipt are ready for the user to return to Claude
Fable's existing “8 FILES” conversation for scoring. Detailed files remain outside
Git. No code, dictionary, references, fixtures or thresholds changed; unchanged
Cargo checks were not repeated. The challenge safeguard failure remains in force.

### A1 scoring receipt and investigation closure — 2026-10-04

Received the non-revealing private scoring receipt. Verified the user's saved
input, output and handoff receipt against the previously recorded hashes, checked
24 exact echoed inputs in order and 120 completed checks, and matched the receipt's
freeze/ledger hashes to the earlier handoff. The original temporary archive is
no longer present; the downloaded run artifacts remain intact. The full archived
executable/build manifest could not be reverified at closure. No analyzer rerun or
private answer-ledger access occurred.

Custodian-reported scoring: 12/12 positive Pass, 12/12 negative Fail, 120/120 check
judgments matching, zero execution errors/missing checks/NotRun. All 12 negative
reason classes match; 11/12 decisive spans match exactly. The remaining nested
span is retained as a discrepancy without changing the frozen reference or giving
exact-match credit. All held-out thresholds pass with the strict 11/12 count.
Source judgments remain model-assisted and provisional; the ledger comparison
is attributed to its custodian rather than independently rescored here.

The investigation closes with **no-go** because the separate visible challenge
safeguard has three unsupported Pass results. No thresholds, code or fixtures
changed, no new model review was requested, and no post-score tuning occurred.
Only aggregate documentation changed; `git diff --check` passed. Prior locked
engineering gates apply to the unchanged implementation and were not repeated.
Private fixtures/reports remain outside Git; scoring alone does not release them.
Any future repair is a new recorded revision, and this scored holdout cannot be
claimed as unseen evidence for it.

### PR #5 review follow-up — 2026-10-04

Addressed both Greptile findings after the completed A1 checkpoint `f1237d4`:
partial dictionary/notice publication and a stale instruction to run the already
scored holdout. PR #6's linked review had no actionable evaluator finding. These
changes preserve the completed no-go, the 11/12 exact-span discrepancy, the pinned
analyzer/dictionary, all public synthetic JSON and the frozen evaluation record.
No scored holdout or visible benchmark was rerun; no private evidence or learner
data was accessed, and no further linguistic review was started.

The installer now verifies exact size/SHA-256 pins for all three bundle files on
extraction and reuse. It synchronizes a complete bundle before publishing one
atomic `current` symlink; existing bundles and legacy flat files remain untouched.
Pre-publication failures preserve the previous bundle. A failed directory sync
after publication reports uncertain durability with the complete new bundle
visible. Tests and examples use `target/a1/current/system_core.dic`; setup remains
explicit and outside Rust runtime composition. The two notice pins were measured
from a fresh temporary copy of the already-pinned official archive.

Strict Red–Green–Refactor evidence:

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Corrupt notice reuse | A same-size damaged LEGAL file was retained and reported ready. | Verify every cached file's size/hash; the test passed. Reviewed pins, naming and isolated test configuration; no further abstraction was justified. |
| Complete bundle publication | An injected final replacement failure left new notices beside the old dictionary. | Publish a complete bundle through one symlink replacement; the test passed, including retention of the old bundle after a successful retry. Reviewed reader lifetime and layout, tightened the fault boundary and retained Python 3.8 compatibility; reran green. |
| Extracted notice verification | An unexpected notice in an otherwise fixture-pinned archive was accepted. | Share bundle verification before publication and reuse; the test passed. Reviewed duplicate checks and error naming; no further change was justified. |
| Publication durability | Pre/post-publication sync-failure tests observed no error because no sync occurred. | Sync files/directories and distinguish post-publication uncertainty; both tests passed. Reviewed descriptor closure, ancestor persistence and bundle retention; clarified names/comments and reran green. |

Nine offline Python tests use tiny synthetic ZIPs and real isolated filesystem
operations, with injected replacement/sync failures at the OS boundary. They also
cover missing/corrupt notices, valid reuse without an archive, invalid archives,
legacy file preservation and a real failed rename. They do not simulate power
loss. CI runs these before installing the real dictionary for Rust adapter tests.

The review/custodian prompt status, protocol status and fixture README now state
that the single run and scoring are complete. Historical copyable prompt bodies
are byte-for-byte unchanged; these wrappers do not authorize another review/run.

Local verification on macOS arm64 / Rust 1.98.1 / Python 3.14.8:

- Nine Python installer tests passed; both scripts also parsed with Python 3.8 syntax.
- Real setup from the exact pinned ZIP and subsequent verified cache reuse passed;
  the retained legacy dictionary/notices matched the current bundle byte-for-byte.
- `cargo fmt --check`, `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`,
  and `cargo test --locked --offline --all` passed: 122 Rust tests plus one rustdoc test.
- Reviewed the complete diff, including the new Python test file; `git diff --check`
  passed. Hosted CI results are recorded separately on the PR.

## A1 follow-up — object-combination safeguard, 2026-10-04

Fetched origin before implementation; it remained at `4931913`. Preserved all
existing files and branches. At the user's request, reviewed and committed the
unchanged completed A1 baseline as local checkpoint `f1237d4`, excluding the saved
follow-up test edits. Private evidence, held-out files, learner data, dictionaries
and build artifacts are outside the commit. No push was requested or performed.

The separate follow-up retains Inconclusive for an object/predicate combination
even with a transitive vocabulary binding. Existing permission failures and their
spans survive. Confirmed RED, minimal GREEN, explicit refactor review and focused
reruns are recorded in [A1 follow-up](docs/A1_FOLLOWUP.md). The unchanged visible
packet now produces 10 Pass, 13 Fail and 13 Inconclusive across 36 cases, with all
180 checks completed and no execution errors. The three unsupported Pass cases
are unresolved; two ordinary development positives also lose Pass. This is an
explicit coverage tradeoff, not new linguistic evidence or a repaired A1 score.

Verification passed on native macOS/arm64 with the repository toolchain:

- `cargo fmt --check`.
- `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`.
- `cargo test --locked --offline --all`: 124 test entries plus one rustdoc,
  zero failures or ignored tests. Local mock-server ports were explicitly allowed;
  dependencies and A1 analysis stayed offline.
- Reviewed the complete follow-up diff, including the new record, for scope,
  ownership, naming, failure precedence and original spans. No further production
  refactor was justified. `git diff --check` passed. Preservation checks confirmed
  unchanged historical evaluation, all frozen public inputs/review packets,
  dependency lockfile and analyzer/configuration pins.

No Linux or hosted CI run is claimed for this revision. Existing live-service and
linguistic-validation limits remain. Rust note: the object evidence is a pair of
borrowed tokens; preserving its span requires no token or sentence copy.

## Offline analyze CLI — separate milestone, 2026-10-04

Started after inspecting the clean local `main` and fetching origin. Both were
`84bc00c6686d1a12257e05e787d62a4a83626533`, containing merged PRs #5 and #6.
Created `codex/offline-analyze-cli` from current `origin/main`; preserved every
existing branch, commit and ignored file, including the installed dictionary.
No reset, stash, clean or dictionary setup/download was needed.

The binary composes `SudachiAnalyzer` and `evaluation::evaluate` synchronously.
Its version-1 JSON input contains only sentence, free-form declarations and
existing bindings plus a version. A 64 KiB read bound and the existing 100-scalar
sentence bound validate external input. Text shows every check, reason and original
span excerpt; JSON preserves the complete analysis, provenance, input and limitations.
Completed Pass/Fail/Inconclusive exit zero. Invalid input and execution errors
produce escaped stderr and no completed evaluation. See SPEC and README for the
exact contract and runnable original synthetic examples.

The research harness, A1 records and frozen public packets remain unchanged.
No private evidence or held-out file was opened, no holdout was rerun or tuned
against, and no new linguistic review, generation or provider integration occurred.
Historical outcomes remain 24/24, checks 120/120, exact negative reason/spans 11/12,
and no-go due to three unsupported visible Pass results. The separate safeguard
retains object-combination uncertainty, including ordinary object sentences,
alongside existing permission Fail findings and their spans.

### Red–Green–Refactor evidence

| Cycle | Observed RED | GREEN and explicit refactor review |
| --- | --- | --- |
| JSON command composition | The subprocess rejected the absent `analyze` command. | Reused the real adapter/evaluator and serialized a borrowed report; direct library values matched exactly. Reviewed ownership, synchronous boundaries and public surface; no new shared API or further refactor justified. Focused rerun passed. |
| Readable checks and spans | Default output was JSON, missing overall/check lines and quoted original excerpts; hostile-input text test also failed. | Added text presentation of all five checks, coverage, findings and limitations, with `escape_debug` on supplied text. Pass/Fail/Inconclusive and control-character tests passed. Reviewed state mapping, span invariants, naming and duplication; no further change justified. Focused rerun passed. |
| Input boundary and diagnostics | Unsupported versions and unknown fields succeeded; missing-field causes were hidden; oversized input lacked the size error. | Reject unsupported versions/unknown fields, expose escaped context chains and bound reads to 65,537 bytes before parsing. Refactor extracted the cohesive private loader and split the test matrix into focused cases sharing error/preservation assertions. Both modes passed again, including exact-limit decoding. |
| Terminal-safe JSON and arguments | JSON emitted raw C1 controls; Clap emitted raw ESC from an unexpected argument. | Escape remaining nonprinting Unicode using JSON UTF-16 escapes, retaining decoded data; sanitize argument errors while preserving help/version success and argument-error exit 2. Reviewed escaping scope, surrogate pairs, string boundaries and ownership; kept serialization in Serde with no new formatter type or dependency. Focused rerun passed. |
| JSON DEL boundary | Refactor review added a regression that exposed raw ASCII DEL in JSON. | Include DEL in the escaped output without changing decoded strings. Reviewed string slicing and allocation; Clippy prompted byte-slice writes instead of slicing strings before conversion. No further change justified. Focused controls tests and the full suite passed. |

The remaining acceptance coverage exercised already-composed behavior without
production changes: missing/unpinned dictionaries, malformed bindings, empty
permissions, object uncertainty together with three permission-failure spans,
100-scalar/64-KiB boundaries, missing HOME/credentials, and poisoned ambient learner
and analyzer files. Refactor review retained the shared real analyzer for direct
calls and isolated subprocess environments, with no mock or new test-only adapter.

### Verification and delivery

Executed on native macOS/arm64 with the pinned Rust 1.98.1:

- `cargo fmt --check` — passed.
- `cargo clippy --locked --offline --all-targets --all-features -- -D warnings` —
  passed after the byte-slice refactor above.
- `cargo test --locked --offline --all` — **137 test entries plus one rustdoc**,
  zero failures or ignored tests. The full run had permission for local loopback
  HTTP mocks; dependencies and analysis stayed offline.
- All 13 `analyze_cli` tests passed with the real pinned dictionary. CLI JSON
  equals direct library analysis/evaluation, including provenance, all checks,
  original text and byte spans. Text and JSON preserve the object safeguard and
  existing permission failures. Error paths publish no evaluation.
- The three exact README analyze commands passed: nominal Pass, unlisted-word
  Fail with original `0..3` excerpt, and object Inconclusive with `6..24` span.
  Existing preview and prepare CLI/library examples, synthetic status and the
  unchanged A1 three-case smoke harness also passed. Temporary preparation/status
  input bytes and directory inventories stayed unchanged.
- Help and version succeeded under `env -i`. Markdown fences/final newlines,
  49 local links across five changed/new Markdown files, three new JSON fixture
  envelopes, and `git diff --check` passed.
- Reviewed all source, tests, fixtures and documentation for synchronous data flow,
  ownership, public surface, escaping and scope. The library, dependency/toolchain
  pins, all A1 records/frozen packets and the synthetic harness match origin/main.
  Only code, tests, original public synthetic fixtures and documentation belong
  in the delivery commits; no dictionary, learner state or build output is staged.

This revision is delivered on `codex/offline-analyze-cli` for review against main;
it is not a merge authorization. No Linux or hosted CI result is claimed in this
local verification record. No private evidence, scored holdout, learner cache,
credentials or external service was used. Tests never skip or substitute the
real adapter. Contract fixtures have separate CC0 provenance and supply no new
linguistic accuracy evidence. Naturalness, MWE and contextual reading/sense remain
unassessed; the historical A1 no-go and exact-span discrepancy stand.

Rust notes for a Ruby developer: the binary-private `Report<'a>` borrows input
while owning analysis/evaluation results, avoiding copies of the input graph.
Existing enum states serialize without collapsing uncertainty into a boolean;
`Result` carries execution errors separately. Ordinary synchronous composition
needs no service object, trait registry or async runtime.

### PR #7 review follow-up — argument diagnostics, 2026-10-04

Greptile's summary and inline comment report the same valid readability issue:
escaping the entire Clap error turns diagnostic newlines into literal `\n`.
The regression first failed on the missing real newline before `Usage:`. Escaping
the invalid argument/value/subcommand contexts before Clap formats the error made
it green, retaining escaped hostile input, stderr-only output and exit 2. Explicit
refactor review found no justified production change; the focused rerun passed.

Review then checked the executable name, which Clap also inserts into usage. A
separate Unix `argv[0]` regression failed with raw hostile content in usage; the
fixed `yomibu` binary name made it green. Refactor review reused the error assertion's
decoded string instead of decoding stderr again. Focused reruns passed, including
invalid subcommands, numeric values, policy values, missing-argument indentation
and normal help. The helper consumes the concrete Clap error without cloning it;
no parser, evaluator, dependency or public library API was added.

Verification passed on native macOS/arm64 with the pinned toolchain:
`cargo fmt --check`,
`cargo clippy --locked --offline --all-targets --all-features -- -D warnings`,
and `cargo test --locked --offline --all` (139 test entries plus one rustdoc;
zero failures or ignored tests). The suite used the real pinned dictionary and
isolated loopback HTTP mocks. Manual `analyze` missing-arguments, `analyze --help`
and `--version` checks under an empty environment confirmed readable layout,
correct streams and exit statuses. Full follow-up diff review and
`git diff --check` passed. A1 records, fixtures and evaluator judgments remain
unchanged. No Linux or hosted CI result is claimed for this follow-up.

## Completed milestone records

### 1a — Offline status

- Initialize one package with library and thin binary, the verified stable
  toolchain pin, edition 2024, and committed lockfile.
- Introduce domain structures and the versioned cache reader. Normalize absence
  and subject variants explicitly; preserve source state without a "known" policy.
- Implement real offline summaries and CLI status, including `--data-dir`.
- Exercise fixtures through production loading and summary code. Fixtures are
  test inputs, not a fake application data source.
- Verify empty accounts, missing/corrupt/unsupported caches, and correct aggregate
  review arithmetic. Status must work without credentials or any network access.

#### 1a delivery and TDD record

Each cycle below ran its tests before implementing the behavior, observed the
expected failure, implemented the change, confirmed GREEN, explicitly reviewed
production and test code, and reran the focused suite after that review. The
initial package/toolchain and empty targets were non-behavioral scaffolding.

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Cache loading | Missing cache API prevented the empty-account test from compiling | Loaded the fixture with Unicode, parsed timestamps, and nullable learner state; reviewed ownership/naming, no refactor justified |
| Cache errors/version | Four tests exposed generic I/O/JSON errors and payload decoding before version rejection | Added distinct typed errors with remedies and version-first decoding; reviewed error paths and kept path clones confined to failures |
| Retained source state | Mixed-fixture test failed to compile because subjects/progress and variants were absent | Added the three lexical shapes, assignments, statistics, and explicit content exclusions; reviewed absence and modelling, no abstraction justified |
| Cache integrity | All supplied invalid-value, reference, and duplicate mutations were incorrectly accepted | Added synchronous validation; refactored test mutation checks to remove panic-catching, then reran all cache tests |
| Counts | Summary API absent | Empty and mixed counts passed; reviewed deduplication, borrowed metadata, and inclusive vocabulary counts, no change justified |
| SRS grouping | SRS summary field absent | Grouped raw stages by optional system ID; reviewed deterministic ordering and unknown-system handling, retained concrete ordered maps |
| Accuracy | Aggregate accuracy API absent | Counter-weighted accuracy, independent zero denominators, and large counters passed; reviewed widening and encapsulated totals, no change justified |
| CLI status | No-op binary emitted nothing and incorrectly succeeded on cache errors | Real fixture-backed output and nonzero errors passed; reviewed presentation/library separation and shared accuracy rendering, no further change justified |
| HOME resolution | CLI required an explicit path even with HOME and lacked the unavailable-HOME remedy | Added lazy OS-native HOME fallback and rejected empty HOME; reviewed precedence and subprocess environment isolation, no change justified |
| Reading classification regression | Final review found that a blank kanji reading kind was accepted; regression test failed as expected | Required nonblank source classification; reviewed the small guard and test, no further refactor justified |

Quality gates executed successfully on macOS/aarch64 with Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` — 21 integration tests passed, including table-driven invalid
  cache cases; no ignored tests.

Also inspected both help screens and the normal dependency graph. Source review
confirmed the only environment read is HOME in the binary, no production
`unwrap()`/`expect()` or unsafe code, and no HTTP client/runtime. CLI tests use
child-process `env_clear()` and isolated temporary directories; they do not
modify process-global environment. Fixture provenance and count semantics are
documented in `tests/fixtures/README.md` and `SPEC.md`.

Linux execution and CI remain unverified/deferred to 1d. No live account, real
API call, synchronization, persistence, or locking behavior was implemented or
claimed tested. The initial sandboxed dependency download could not resolve
crates.io; the authorized network-enabled Cargo run resolved dependencies, and
subsequent tests and quality gates succeeded locally.

### 1b — First complete sync

- Fetch and normalize learner identity, progress, statistics, and associated
  subjects using the actual HTTP adapter against a local mock server.
- Implement full snapshot replacement, account protection, and safe cache writes.
- Keep command composition in the binary and useful behavior callable from the
  library. Do not leak transport DTOs into the public API.
- Verify the complete retrieval-to-cache-to-status flow, including repeated syncs.

#### 1b delivery and TDD record

Behavior was developed in Red–Green–Refactor cycles. Each row records the observed
RED, the implemented GREEN, and explicit production/test refactor review followed
by a focused rerun. Dependency declarations, synthetic source fixtures, and test
module setup were test-enabling scaffolding.

| Cycle | Observed RED | GREEN and refactor review |
| --- | --- | --- |
| Private full replacement | `SyncGuard` absent, round-trip test did not compile | Added serialization, lock ownership, private directories/files, temporary-file flush/sync, atomic persist, and directory sync; reviewed ownership/error stages, no abstraction justified |
| Cache/account protection | A corrupt existing cache was overwritten | Validate existing caches and new snapshots; refuse account mismatch; refactored shared existing-cache loading and reran all cache tests; basic lock contention/read coexistence also passed |
| HTTP foundation | `Client` absent | Real bearer/revision requests normalize an empty profile; authentication, redirects, malformed JSON, and oversized responses fail safely; reviewed sanitized errors and bounds, no further change justified |
| Source normalization | Nonempty assignments returned `InvalidResponse` | Private typed DTOs normalize all three lexical variants, progress, and access exclusions; exact normalized mixed fixture matches; refactored progress iteration to avoid an intermediate allocation |
| Complete pagination/batching | Empty first pages lost progress; 101-ID request missed bounded mocks; unsafe-page test exposed a later-endpoint false positive and was tightened to fail specifically on pagination | Explicit termination, origin/path/credential checks, repeated-URL detection, and 100-ID batches passed; reviewed ordering and termination, no further refactor justified |
| Source integrity | Identical duplicates failed; invalid excluded levels/content and missing nullable fields were accepted | Collapse equal retained records, reject conflicts, require nullable source fields, validate content before exclusion; extracted shared subject validation and a small deduplication function for three collections |
| Retry foundation | Retry/reset helpers absent | Actual HTTP retries obey the two-retry budget and excessive resets fail; fixed-time tests verify 1/2-second backoffs and reset/default calculations; removed an unnecessary unreachable panic branch |
| CLI command/guidance | `sync` unrecognized; missing-cache text still said sync unavailable | Environment-only token handling, synchronous lock/write around async retrieval, and shared status output; reviewed branch-local environment/runtime use and reran CLI tests |
| Cross-component acceptance | Local-origin constructor private; constructor accepted unsafe remote HTTP | Validated library origin configuration; complete HTTP → cache → offline subprocess, repeat refresh/removal, and different-account preservation passed; reviewed the small public API and kept endpoint configuration out of CLI options |
| CLI composition | Extracted composition function absent | Tested the actual binary composition with mock HTTP, asserted the lock is held during requests, rendered and reloaded the same snapshot; reviewed runtime/guard lifetimes, no further refactor justified |

Quality gates passed on macOS/aarch64 with Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` — 39 tests passed (11 library HTTP tests, 1 binary composition
  test, 13 cache, 7 CLI, 6 summary, and 1 cross-component test); none ignored.
- `git diff --check`; reviewed new/untracked source and fixture files as well.

Also inspected CLI and sync help and the normal dependency graph. Production has
no `unwrap()`/`expect()`, unsafe code, credential-bearing error sources, DTO exports,
or token/config persistence. The only environment reads are HOME and the sync
branch's token read in the binary. Runtime startup, argument parsing, output, and
exit status remain there; the library owns HTTP, normalization, cache operations,
and summaries. Dependencies have only the required features enabled.

Sandboxed Cargo could not resolve crates.io or bind local listening ports. The
approved Cargo runs downloaded dependencies and ran loopback mocks successfully;
normal tests never contact the real WaniKani service. An attempted paused Tokio
clock advanced past real socket I/O, so retry delay calculations use a fixed input
clock and HTTP retry classification/budget tests use real loopback I/O (about six
seconds for bounded backoffs). No wall-clock timing thresholds are asserted.

At the 1b stopping point, no live account/API run, Linux execution, CI, expanded
streamed-body/deadline cases, deterministic fault injection around replacement/
directory sync, or cross-process interruption matrix had been verified. The
post-replacement error category existed without an induced directory-sync failure.
The 1c results below address the resilience gaps; Linux/CI remain in 1d.

### 1c — Sync resilience

- Expand verification and harden pagination, bounded retries, deadlines, streamed
  response limits, credential-safe URL handling, and structural integrity.
  Required safety foundations and representative tests were introduced in 1b.
- Extend the 1b reset/removal, permitted absence, access-exclusion, and contention
  cases to failures on later pages, cross-process contention, and interruptions.
- Verify old-cache preservation on every tested pre-replacement failure. Test
  post-replacement durability errors separately rather than asserting rollback.

#### 1c delivery and TDD record

The cancellation fix followed a confirmed behavioral RED before any production
change. Test-enabling private seams were introduced only after their focused
tests failed to compile. Coverage extensions for guarantees already implemented
in 1b passed without production changes. Each group received an explicit review
of production/test naming, duplication, modelling, ownership, and Rust idioms,
followed by focused reruns and the full gates.

| Cycle / verification group | Observed RED or existing behavior | GREEN and refactor review |
| --- | --- | --- |
| Cancelled rate-limit wait | Polling and dropping a fetch erased the pending deadline (`None` instead of the saved instant) | Clear the deadline only after the wait; focused regression passed; reviewed borrowing and cancellation, added the invariant comment, then reran |
| Storage fault boundaries | Tests could not compile without private checkpoints | Added one private generic callback around the existing write sequence; pre-replacement failures preserve bytes and parsed data, while directory failure exposes the complete new snapshot with `DurabilityUncertain`; consolidated the directory error mapping and staging-file lookup, then reran |
| HTTP deadlines | Focused stalled-response test could not compile without a private timeout constructor | Factored construction while keeping public defaults; 250 ms test deadlines cover headers and body and exhaust exactly three attempts; refactored shared cache-preservation assertions, then reran |
| Streamed bodies and retries | Existing 1b behavior passed the new cases | Exact 16 MiB chunked body succeeds; excess chunked/close-delimited bodies and declared oversize fail; truncated length/chunk framing exhaust retries; successful recovery discards partial bytes; shared the raw server's bounded response sequence and reviewed task cleanup |
| Rate timing, URL safety, and structural integrity | Existing guards passed expanded cases | Controlled time proves successful and 429 reset headers delay reused clients; fixed-clock reset boundaries, permanent errors, mixed transient budgets, hostile URLs, redirects, invalid excluded data, and broken references are covered; reviewed sanitized error chains and test-only helper visibility, no further abstraction justified |
| Later-page failures and changing state | Existing replacement rules passed the expanded matrix | Authentication, malformed JSON, server/rate errors, missing terminators, conflicts, and cycles on all three collections preserve cache bytes and offline output; access changes and removal of statistics/review-only content replace old state; reviewed fixture isolation and absence assertions, no production change justified |
| Process interruption and contention | Existing locking/replacement passed separate-process checks | Kill during fetch and at six write boundaries, including a partial staging file; verify complete old/new reads, private staging files, stable lock inode, release/reacquisition, and later replacement; real CLI contention fails while status works; reviewed readiness handshakes and RAII child cleanup, cleared child environments, and shared staging-file lookup, then reran |
| Actual rename failure | Existing error mapping passed a forced missing-source rename | The real persist operation fails after staging-file removal while old bytes and parsed data survive; reviewed cleanup and error classification, no further change justified |

The subsequent simplification pass merged overlapping HTTP integrity,
pagination, reset, and storage-fault matrices, removing 103 test lines while
retaining each distinct safety scenario. APIs and dependencies are unchanged.
Focused suites and full gates were rerun. The child helper now parks until killed;
both interruption tests require SIGKILL rather than accepting any failed exit.

Parallel validation exposed a lock-release edge case. A deterministic regression
with a duplicated lock-file handle failed with `Locked` after dropping its guard.
Explicit unlock in `SyncGuard::drop` made it pass; closing alone may retain the
lock through a briefly inherited descriptor during process creation. Refactor
review renamed `_lock` to `lock`, kept cleanup local to `Drop`, and verified that
dropping the old duplicate cannot unlock a subsequent guard. Focused storage
tests and all gates were rerun after that review.

Quality gates passed on macOS/arm64 with the pinned Rust 1.98.1:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` — 56 test entries passed: 23 library, 1 binary composition,
  13 cache, 8 CLI, 6 summary, and 5 cross-component entries. Two entries are
  subprocess entry points; table-driven tests cover multiple failure scenarios.
  None ignored.
- `git diff --check`; reviewed the new test files as well as tracked diffs.

The first sandboxed baseline run could not bind loopback ports. Authorized test
runs used only local servers and synthetic credentials. No live API request was
made. Test processes receive their own environment overrides; no process-global
variables are mutated. Existing 1/2-second retry integration backoffs still use
real time, but rate-reset timing uses explicit clock advancement, with time
resumed before real socket I/O. Short deadline tests assert outcomes and request
counts, not elapsed wall-clock thresholds.

Fault injection is at storage operation boundaries, with a real rename-failure
case; it does not emulate failing hardware or power loss. Process-kill tests show
atomic visibility and lock recovery, not survival of a machine crash. Killed
writers may leave private staging files, which subsequent reads/writes ignore.
Connect-timeout configuration remains 10 seconds; DNS/TLS blackholes and live
service behavior are not separately simulated. Linux execution, CI, and final
public-surface acceptance remain in 1d. No deferred product features, new public
APIs, cache schema changes, or new crates were added.

Rust notes for a Ruby developer: a dropped async future stops at an `await`, so
state needed by the next call must remain owned by the client until that wait
completes. `impl FnMut` provides a small private, statically dispatched test seam
without a public adapter hierarchy. `Drop` releases normal-scope resources, while
process-kill tests separately verify the OS's file-lock cleanup when destructors
do not run.

### 1d — Milestone acceptance

- Run the quality gates on macOS and Linux and enforce the lockfile in CI.
- Review the public library surface, dependency features, secret handling, error
  messages, and alignment with `SPEC.md`.
- Document actual validation results and remaining limitations. Do not make a live
  account or a real API request a prerequisite for the automated test suite.

#### 1d delivery and TDD record

Added `.github/workflows/ci.yml`: push and pull-request events run one matrix on
`ubuntu-latest` and `macos-latest`, with independent results and a 20-minute job
deadline. Rustup reads the exact version and components from
`rust-toolchain.toml`; there is no second toolchain pin. Each job verifies that
`Cargo.lock` is tracked, runs formatting, Clippy, and tests with lockfile
enforcement, then checks that the lockfile is unchanged. Checkout is pinned to
the verified v7.0.1 commit, with read-only contents permission and credential
persistence disabled. CI requires no WaniKani credential or live account.
The initial workflow introduced no extra build scripts, caching layer, or testing
infrastructure. The caching follow-up is recorded below.

Reviewed the workflow against the official
[checkout documentation](https://github.com/actions/checkout/tree/v7.0.1),
[workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax),
and [Rustup toolchain-file behavior](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file).
The Linux run also confirmed that `rustup show active-toolchain` installs the
file's missing rustfmt/Clippy components.

| Review area | Finding and disposition |
| --- | --- |
| Public library surface | Retained the four concrete modules and existing signatures. Added rustdoc for validated public data, borrowed summaries, lock ownership, trusted custom origins, Tokio requirements, client reuse/cancellation, and replacement outcomes. Transport DTOs and fault/timeout seams remain private. |
| Dependency features | Inspected normal/build and feature graphs: reqwest uses Rustls with defaults off; clap and chrono retain their narrow feature sets; Tokio directly enables `rt`, `time`, and `net`, with `io-util`/`sync` also required transitively. Development features support existing HTTP/clock tests. No `native-tls`, unnecessary direct feature, new crate, or lockfile change was found or introduced. |
| Secret handling | The binary alone reads HOME and the sync token; status still needs neither credentials nor an HTTP client. Authorization is marked sensitive, the client has no Debug/Serialize implementation, and HTTP errors discard URLs/bodies/transport causes. Existing hostile-URL, redirect, malformed-body, error-chain, and CLI tests passed with synthetic tokens. No credential/config persistence was added. |
| Errors and recovery | Existing messages distinguish missing/corrupt/unsupported caches, authentication, lock contention, account mismatch, pre-replacement failures, and uncertain durability. Found and fixed the missing I/O cause on `DurabilityUncertain`, retaining its variant and message. |
| Specification and scope | Cache schema, source-state semantics, CLI options, HTTP limits, and synchronous/async boundaries remain unchanged. No future product feature or abstraction was added. |

The only behavioral change followed strict Red–Green–Refactor:

- **RED:** extended the existing storage-fault matrix to inspect the underlying
  `io::Error` and its kind. The focused test failed specifically at
  `SyncDirectory` with “missing I/O cause”; earlier fault boundaries passed.
- **GREEN:** marked the existing `DurabilityUncertain` field with `#[source]`.
  The same focused test passed, including actual rename failure and old/new
  cache preservation assertions.
- **REFACTOR:** reviewed production and test naming, modelling, duplication,
  ownership/borrowing, and Rust idioms. Reused the fault matrix and private seam;
  no additional code abstraction or behavior change was justified. Placed API
  documentation before derives and reran the focused test successfully.

CI and documentation changes are non-behavioral scaffolding and did not receive
artificial failing tests.

Validation executed on 2026-09-28 with pinned Rust/Cargo 1.98.1:

| Environment | Formatting | Clippy | Tests |
| --- | --- | --- | --- |
| Native macOS/arm64 | Passed | Passed, warnings denied | 56 entries passed, none ignored |
| Linux/aarch64, Debian 12 container under OrbStack | Passed | Passed, warnings denied | 56 entries passed, none ignored |

Both environments ran `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`, and
`cargo test --locked --all`. The 56 entries comprise 23 library, 1 binary, 13
cache, 8 CLI, 6 summary, and 5 cross-component tests, including two subprocess
entry points. Linux used the official `rust:1.98.1-slim-bookworm` image, copying
source from a read-only mount into its own filesystem; the final lockfile matched
the repository byte for byte. Native tests used authorized loopback networking.

Additional checks passed:

- Actionlint 1.7.12, downloaded to a temporary directory from its official release
  and checked against the release checksum, reported no workflow errors.
- Temporary manifest copies with missing and stale lockfiles each failed Clippy
  with exit 101 specifically because of `--locked`; neither lockfile was created
  or changed. The repository manifest and lockfile were untouched.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` built the library
  documentation without warnings; inspected top-level, sync, and status help.
- Reviewed production code for public exports, environment reads, dynamic errors,
  unsafe code, and recoverable `unwrap()`/`expect()`; no additional findings.
- `git diff --check`, review of the new workflow, and unchanged manifest,
  toolchain, and lockfile checks passed.

At initial delivery, hosted runs were unverified; the subsequent successful
Ubuntu/x86_64 and macOS/arm64 run is recorded below. No live WaniKani request,
DNS/TLS-blackhole simulation, hardware-fault test, or power-loss
durability test was performed. Existing process-kill/fault-injection limitations
from 1c still apply. Per-request deadlines and page-size bounds do not impose a
total refresh deadline or collection-size bound; this existing API limit is now
documented. No additional platform support is claimed.

Rust notes for a Ruby developer: formatting an inner error in `Display` does not
automatically expose it through `std::error::Error::source()`. `#[source]` makes
the cause inspectable while keeping the meaningful outer enum variant. Cargo
features are additive across dependencies, so the resolved feature graph matters
as well as each direct dependency declaration. `--locked` refuses resolution
changes rather than silently editing the dependency snapshot.

#### CI caching and scheduling follow-up

The [first hosted run](https://github.com/kalorz/yomibu/actions/runs/36390858621)
passed every gate and all 56 test entries on both platforms. It took 3m 10s
overall: Linux ran for 1m 54s and macOS for 3m 02s, starting four seconds apart.
Actual test execution took about 19–21 seconds per OS; dependency checking and
compilation dominated the run. No cache was restored or saved in that workflow.

The follow-up pins `Swatinem/rust-cache` v2.9.2 to its verified release commit,
after toolchain selection and the tracked-lockfile check. It reuses dependency
downloads and compiled dependencies with the action's platform/compiler/manifest
keys. Only `main` saves caches; PRs can restore the base branch's caches. Cold
builds still run all gates. The Rust toolchain itself is not cached by this step.
See the [action's cache contract](https://github.com/Swatinem/rust-cache/tree/v2.9.2).

Push events are restricted to `main`, while pull-request events cover proposed
changes, avoiding duplicate branch-push/PR runs. Workflow-level concurrency
cancels superseded runs for the same ref without serializing the OS matrix.
Documentation-only skipping remains deferred; both OS jobs retain every gate.
These CI/documentation edits are exempt from artificial behavioral RED tests.
Review kept the existing matrix and commands without a new script or helper.

Validation on 2026-09-28: actionlint 1.7.12 and `git diff --check` passed.
The [cold attempt](https://github.com/kalorz/yomibu/actions/runs/36393246817/attempts/1)
and [warm rerun](https://github.com/kalorz/yomibu/actions/runs/36393246817/attempts/2)
of commit `04db3a7` both passed formatting, locked Clippy with warnings denied,
all 56 test entries per OS, and unchanged-lockfile checks. The warm logs confirm
exact cache hits with distinct Linux/x64 and Darwin/arm64 keys.

| Run | Linux job | macOS job | Overall elapsed |
| --- | --- | --- | --- |
| Original uncached workflow | 1m 54s | 3m 02s | 3m 10s |
| New workflow, populating caches | 1m 35s | 2m 34s | 2m 41s |
| Same commit, warm caches | 45s | 54s | 1m 04s |

Warm Clippy took about 2s/4s and test compilation 6s/8s on Linux/macOS;
the full tests still executed. These are individual measurements, not a runtime
guarantee: queueing, runner changes, dependency/toolchain changes, and cache
eviction affect later runs. PR scheduling and cancellation were linted/reviewed,
without creating a synthetic PR or deliberately overlapping runs. No Rust source,
dependencies, lockfile, or test coverage changed.

Rust note for a Ruby developer: this cache retains compiled dependency artifacts
as well as downloads. Their compatibility depends on the compiler and target
platform, so Linux and macOS need separate cache entries.

#### Test execution follow-up

The slow tests spent most of their time in real retry waits of one and two seconds.
Three independent endpoint matrices serialized those waits, as did pairs of stalled
and truncated response scenarios. The follow-up uses existing Tokio `join!` and
local async closures to overlap each group, with at most three independent
servers/caches in a group. The successful HTTP retry and exhausted retry cases
are now separate tests. All 21 later-page failure cases retain their request-count,
error, credential, cache-byte, parsed-state, offline-status, and lock assertions.
Real socket tests retain their real timers, stalled requests, and retry budgets.

For exact transient timing, a socket-free paused-clock test replaces the two
duration-value assertions. It verifies that each wait is pending immediately
before its deadline (one or two seconds) and completes at the deadline. The existing
private duration helper now owns the existing sleep; production retry behavior stays the
same. No test-only delay configuration, dependency, public API, or runner was added.
Paused clocks are not used while real socket I/O is pending.

- **RED:** added the focused timer test first; it failed to compile because the
  private wait helper did not exist.
- **GREEN:** moved the existing sleep into that helper; the focused test passed
  in 0.00s.
- **REFACTOR:** reviewed naming, duplication, ownership, case isolation, cleanup,
  and concurrency bounds. Local closures reuse each matrix's assertions; split
  retry tests distinguish recovery from exhaustion, and the reset test now names
  its specific responsibility. No further production abstraction was justified.
  The focused HTTP suite (21 entries) and later-page matrix passed after review.
  Test scheduling changes are infrastructure refactors and need no artificial
  behavioral failure.

Validation on 2026-09-28: native macOS/arm64 and a disposable Debian 12
Linux/arm64 container passed `cargo fmt --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`, and
`cargo test --locked --all`. All 58 entries passed, none ignored: 25 library,
1 binary, 13 cache, 8 CLI, 6 summary, and 5 cross-component entries, including
the same two subprocess helpers. The increase comes from one new timer test
and splitting an existing test; no scenario was removed. `git diff --check`
also passed. The container used the pinned Rust 1.98.1 image and a read-only
source mount, and its lockfile remained byte-identical to the committed file.

| Native macOS execution | Before | After |
| --- | --- | --- |
| Library tests | 7.57s | 3.80s |
| Cross-component tests | 9.55s | 3.85s |
| All test binaries, summed reported execution | 17.84s | 8.10s |

The Linux container reported 4.23s for the library and 3.88s for cross-component
tests, or 8.31s summed across all test binaries after the change.

The [hosted run for `8757ade`](https://github.com/kalorz/yomibu/actions/runs/36445060921)
passed every gate and all 58 entries on Ubuntu/x86_64 and macOS/arm64. Both jobs
restored exact dependency-cache hits. Summed test execution was 10.54s on Linux
and 10.64s on macOS, excluding test compilation of 6.37s and 12.15s respectively.
Clippy took 2.16s/5.61s; toolchain setup and cache restoration still contributed
about 12s/16s. The OS jobs overlapped.

| Hosted run | Linux job | macOS job | Overall elapsed |
| --- | --- | --- | --- |
| [Previous commit, `6f79d1b`](https://github.com/kalorz/yomibu/actions/runs/36393872248) | 46s | 1m 09s | 1m 19s |
| Test follow-up, `8757ade` | 36s | 55s | 1m 06s |

The follow-up did not achieve a workflow below 45s or 30s. An earlier warm run
already finished in 1m 04s, illustrating the setup/runner variation even though
test execution is now roughly halved. No fixed hosted runtime is promised.

These single-run measurements exclude compilation and process startup; they are
not timing assertions or a CI runtime guarantee. Real backoffs remain a floor
for HTTP tests, and hosted setup, cache restoration, compilation, and scheduling
still take time. Existing live-service, DNS/TLS, and power-loss verification
limits remain. Product work stops at milestone 1d.

Rust note for a Ruby developer: `join!` polls independent futures on the same
runtime, so one case can progress while another waits. Each future owns its
server and temporary directory; unwinding still drops those resources. This
reduces serialized waiting without weakening the real HTTP boundary checks.

## Naming and design follow-up — 2026-09-30

Accepted the responsibility vocabulary in `SPEC.md`, including
`SourceConnection`, `LearnerProgress`, `LearnerKnowledge`,
`LearnerKnowledgePolicy`, `PromptTemplate`, `ModelRequest`, `PromptStore`,
`CandidateGenerator`, and `ExerciseGenerator`. Future adapters use the
`InMemory...` prefix. These names define a direction, not a list of scaffolding
to add. Grammar entries retain provider identity; no cross-provider grammar
ontology is required. Code may remain public while Cloud supplies private data.

Renamed the implemented `domain::Snapshot` to `domain::WaniKaniSyncData`, matching
its account scope and synchronization interval. Renamed the validation-error
variants from `InvalidSnapshot` to `InvalidSyncData`, updated callers, local
variables, test names, and documentation. This changes Rust source API names and
diagnostic wording; it does not change validation, network requests, or storage
behavior. No compatibility alias is retained in this unpublished library.
The private persistence envelope keeps its `snapshot` field and schema version 1;
existing JSON fixtures remain unchanged.

The future composition retains explicit dependencies and optional sync. Direct
in-memory data is enough for one-off generation, local CLI storage remains
file-backed, and Cloud selects its stores. `LearnerKnowledge` is derived on
demand. Separate writes from reads without adding event sourcing, a command bus,
or a separate read database. No future types, traits, adapters, or crates were
implemented in this follow-up.

This is a naming refactor and documentation update, with no new behavioral path
requiring an artificial failing test. Before the rename, all 19 existing cache
and summary integration tests passed.

Refactor review covered production and test naming, responsibilities, duplication,
ownership/borrowing, and Rust idioms. Kept the existing modules, owned sync data,
borrowed summaries, private persistence envelope, and behavioral assertions;
no further abstraction or refactor was justified. One missed test-variable rename
was found by compilation and corrected before the final checks.

Post-change verification on native macOS passed:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all` — all 58 entries passed, including the two
  subprocess helpers; none ignored. The first sandboxed attempt could not bind
  loopback sockets; the authorized rerun passed with local test servers.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`
- `git diff --check` and review of the complete diff; schema fixtures, dependency
  manifest, lockfile, and toolchain pin remain unchanged.

Existing fixture reads, writer round trips, and HTTP-to-cache-to-CLI tests verify
schema-1 compatibility. No live API, model, Linux, or hosted CI run was performed
for this naming follow-up; earlier platform results remain historical evidence.

Rust note for a Ruby developer: renaming a Rust struct does not rename its JSON
fields. The private cache envelope preserves the disk format independently of
the public type name. Public source code also does not require every Rust type
or test helper to be exported with `pub`.

## Architecture migration — 2026-10-01

Implemented the accepted first slice: existing sync/status behind library `App`,
with explicit source and store dependencies. `App` has no runtime/environment
ownership or current-user state. Status needs no source; sync returns an owned
summary and explicit volatile/durable persistence outcome, preserving typed
source and storage errors. The CLI now composes these use cases instead of
implementing synchronization itself.

Moved WaniKani under `adapters/sources/wanikani` and file persistence under
`adapters/stores/file`. Root `wanikani` and `cache` module paths remain re-exports.
Added `InMemoryLearningStore`, which shares immutable versions only across
explicitly cloned handles, validates account/data invariants, and reserves a
writer across retrieval without holding a data lock during network I/O.
The physical `LearningStore` publishes related material and progress together.
Separate logical material/progress read capabilities await actual generation
queries; a third snapshot repository was not introduced.

The current source/storage contracts intentionally remain scoped to normalized
WaniKani data and one account per store. They do not yet provide a generic
multi-provider ontology, multi-tenant authorization, or asynchronous SQL/remote
storage. `ARCHITECTURE.md` records these limits, all six scenario checks, future
generation/Cloud composition, and public code/private asset repository boundaries.
No new dependency, crate, runtime feature, cache schema, or future placeholder was
added. Source API changes include the new entry points and an owned `Summary`
(no lifetime parameter); only the username string is copied, not source vectors.

### TDD and refactor record

Each new behavioral slice began with the listed failing test. Module relocation,
documentation, and routing existing callers through a tested use case were
refactors rather than artificial behavioral RED cycles.

| Cycle | Observed RED | GREEN and explicit refactor review |
| --- | --- | --- |
| Store publication/read contract | Store adapters and ports absent; shared tests failed to compile | Both stores publish complete versions, old readers retain their version, file construction is lazy and schema 1 persists. Reviewed duplication and ownership; used `Arc` reads without cloning the source graph. Focused tests passed. |
| Memory replacement integrity | Memory accepted an invalid synchronization interval | Validate before publication and reject a different account; shared memory/file assertions verify preservation and subsequent writes. Reviewed responsibilities and error paths; domain validation remains shared and backend commit checks remain local. Focused tests passed. |
| Writer reservation | A second memory handle could reserve a concurrent writer | Added a private shared state and an owned reservation released by `Drop`; data locks cover only reads/publication. Reviewed ownership, cancellation, and naming; no mutex guard crosses an await. Six store tests passed after formatting/refactor review. |
| App and owned results | `App` and `LearningSource` absent | Same sync/status use case passes with both stores, and results outlive the App. Reviewed result ownership; an owned summary avoids a second summary type or cloning complete input data. App/store/summary tests passed. |
| Real source composition | WaniKani client and borrowed client did not implement `LearningSource` | Added delegation to the existing HTTP adapter, including borrowed clients. Real HTTP publishes to memory; existing full sync/failure/process tests now call App. Reviewed shared flow and removed duplicate orchestration from CLI/test helpers; focused App, sync, CLI and binary tests passed. |

Additional acceptance assertions passed for cancelling a polled sync (including
`Send` futures with both stores), source/validation failure preservation, and
locked/corrupt stores preventing retrieval while retaining typed errors. These
verify guarantees already supplied by the composed validation and writer guards;
they are not reported as additional observed RED cycles.

Final refactor review covered all new and moved production/test code, names,
module direction, typed errors, ownership, duplicated flow, and Rust idioms.
Kept concrete domain calculations, private DTOs, schema-1 envelopes, and the
existing file fault/interruption seams. No additional abstraction was justified.
No production panic shortcuts, new global state, or environment mutation were
introduced. Existing source/file tests and the full suite passed after review.

### Verification

Native macOS/arm64 with the pinned Rust 1.98.1 passed:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all`: 69 test entries passed (25 library, 1 binary,
  5 App, 13 cache, 8 CLI, 6 store, 6 summary, 5 sync), including two existing
  subprocess helpers; none ignored. One additional rustdoc example compiled.
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`
- `git diff --check`; reviewed new/untracked files as well as tracked diffs.

Local HTTP tests ran with authorization to bind loopback sockets. No live API,
credentials, model, Linux/container, or hosted CI run was used for this migration.
Earlier platform results are historical only. Real file fault/process tests
remain coverage of their stated boundaries, not a power-loss simulation. Request
deadlines still do not bound the total refresh duration or collection size.

Rust notes for a Ruby developer: generic `App<Store, Source>` checks adapter
contracts at compile time without a dependency container. An owned writer token
releases its reservation on return, failure, or cancellation. `Arc` shares an
immutable version, while an owned summary can outlive both App and source data.

Next steps remain the separately authorized milestones below: learner/progress
and material read models with knowledge derivation, then validated generation
using explicit prompt/model dependencies. This migration does not start them.

## Test strategy

| Technique | Meaningful scenarios |
| --- | --- |
| Unit tests | Domain validation, nullable state, subject variants, summary counts, weighted accuracy, zero denominators |
| Local HTTP adapter tests | Headers, success, multiple/empty pages, kana-only vocabulary, permitted partial data, additive fields, malformed JSON, invalid required fields, authentication errors, rate limiting, transient/permanent failures, timeouts, hostile pagination URLs, oversized responses, duplicate records, missing subjects |
| Storage integration tests | Round trips, Unicode/timestamps, schema rejection, corrupt/truncated files, interrupted writes, atomic replacement, writer contention, account mismatch, errors after replacement |
| Cross-component tests | Complete sync, repeat sync, reset/removal replacement, access exclusions, later-page failure leaving the previous cache unchanged |
| Black-box CLI tests | Help, argument validation, missing token, alternate data directory, status without credentials, errors, exit codes, secret-free output |
| Property tests | Input-order independence, count conservation across subject partitions, bounded aggregate accuracy, normalized-data serialization round trips |
| Golden tests | Stable status output with fixed timestamps only |

Use synthetic but realistic fixtures grounded in the official API contract.
Preserve their provenance without storing real credentials or private learner
data. Isolate servers and temporary directories per test. Use controlled clocks
and deterministic property-test seeds; retain regression cases for non-trivial
bugs. Do not use slow wall-clock sleeps to test retry schedules where controlled
time is sufficient.

Normal tests never call the real WaniKani API. Local loopback mock-server traffic
is allowed. Do not mock pure domain logic or mutate process-global environment
variables to configure parallel tests; pass environment overrides to individual
CLI child processes.

Test behavior and invariants rather than copying implementation logic. Do not add
snapshots, property tests, or coverage targets solely to increase a metric.

For future generation tests, run real knowledge derivation, prompt preparation,
response parsing, and validation with scripted model responses at the model
boundary. Call counts are appropriate when they verify a cost or retry budget;
internal helper-call sequences are not contracts. Scripted responses never serve
as automatic production fallbacks or evidence of real model quality.

Use real in-memory adapters for application tests when their storage semantics
are sufficient. Exercise common invariants across implementations and verify
backend-specific durability and concurrency against the actual backend. Retain
local HTTP and real-file tests; future PostgreSQL tests use an isolated database.
An in-memory store does not test SQL or filesystem behavior. Do not require
`create_null()`, a test mode, or a new trait for every component.

Selected influences: [Testing Without Mocks](https://www.jamesshore.com/v2/projects/nullables/testing-without-mocks)
for behavioral tests with real collaborators and explicit infrastructure
boundaries, and [CQRS](https://martinfowler.com/bliki/CQRS.html) for distinguishing
update models from derived read models. Neither is a mandatory architecture kit.

## Validation commands

For the documentation-only milestone:

- Review the three documents for consistent decisions, sources, and stopping scope.
- Confirm that only the requested documents were created.
- Check whitespace with `git diff --check`; include untracked new files in the
  review rather than assuming a normal diff covers them.
- Do not run Cargo quality gates before a Cargo package exists.

Before declaring an implementation milestone complete, run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Milestone 1d adds CI with equivalent checks on macOS and Linux and lockfile
enforcement; use the same locked commands locally:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
```

Use additional tooling only when it answers a concrete concern. Report which
checks actually ran and their results. The documentation stage had no Cargo
package; the completed 1a checks are recorded above.

## Later milestones

The offline preparation slice and its real-learner acceptance are complete above.
A1 implementation, manual reference review, frozen evaluation and private scoring
are complete with a no-go for the frozen implementation. The separately authorized
object-combination follow-up and the separate offline analyze CLI are recorded
above; G1 now separately authorizes experimental candidates, not validated
generation or a claim of new blind evidence from the scored holdout. Remaining implementation
work requires separate authorization:

1. **Learner constraints and retrieval:** grammar knowledge as learner data,
   initially entered through a local file, an explicit revisable
   `LearnerKnowledgePolicy`, manual targets, and structured lexical retrieval.
   Targets identify the word, intended reading, and intended sense. Acceptance: explainable
   target/context selection from real learner data with no mandatory vector
   search. This bounded acceptance criterion was completed on 2026-10-03 using
   the production cache reader and private learner data, as recorded above.
   When database persistence is introduced, grammar
   belongs alongside other learner data; files may remain import/export. Derive `LearnerKnowledge`
   on demand from preserved progress and manual declarations. Future provider
   grammar identifiers remain independent; no semantic cross-provider mapping
   or canonical catalog is required.
2. **Validated generation:** evidence-backed Japanese analysis and explicit
   pass/fail/inconclusive checks, real best-of-two generation, a minimal combined
   naturalness/coherence review, and bounded repair for sentences and stories.
   Include focused, grounded sense/reading checks when supporting ambiguous
   targets. Acceptance: select an acceptable passage or report failure
   without silently relaxing constraints; length and complexity do not authorize
   unfamiliar vocabulary or kanji. Use the `ExerciseGenerator` responsibility,
   with candidate production behind `CandidateGenerator`, prompt sets from
   `PromptStore`, and an explicitly selected `LanguageModel`. Introduce only the
   substitution points demonstrated by the implementing milestone. Bound model
   invocations per phase and globally, including retries; repaired candidates
   receive fresh analysis and full reassessment.
3. **Reading practice:** reading quizzes with kana/romaji normalization and
   persisted attempts. Acceptance: deterministic comparison against contextually
   validated readings and useful mistake records. Rephrase or reject ambiguous
   prompts; do not penalize another contextually valid reading solely because it
   differs from the intended target.
4. **Meaning and usage evaluation:** multilingual translation judging and grounded
   semantic criticism beyond the focused target checks already introduced.
   Acceptance: evaluate preserved meaning without requiring one canonical
   translation; extend the basic naturalness/coherence review and focused
   reading/sense assessment already introduced with generation.
5. **Adaptive selection:** combine WaniKani statistics with Yomibu mistakes.
   Acceptance: target choices use both sources and can be explained without
   introducing an SRS.

Bunpro, JMdict, external corpora, web/native/MCP interfaces, PostgreSQL, and vector
retrieval remain conditional future work. Revisit them only when a concrete
product or retrieval need justifies their cost.

## Managed dictionary loading — implementation, 2026-10-05

The user approved implementation after reviewing the plan. Started the isolated
`codex/managed-sudachi-dictionary` worktree at `/workspace/yomibu-managed` from
freshly fetched `origin/main` `fce266e882febf764a468c17b9c127c89e3ec766`.
The original `/workspace/yomibu` worktree, `work` branch and ignored artifacts
remain unchanged. No reset, stash, clean or worktree removal occurred. The new
worktree's ignored `target/a1` refers to the existing verified developer bundle;
the Rust importer copies it into separate managed storage.

PR #11 remained open/unmerged at corrected head
`a1e82b5e15e03c9c5466f3b335ea5b5f892a3982`; its Linux/macOS current-head CI run
37331138136 passed. This branch does not change test profiles or that PR's branch.
Repository/session Git identities were checked, with repository-local configuration
kept outside tracked files.

### Final behavior and safety

`dictionary import --bundle PATH [--dictionary-dir PATH]` copies the dictionary
and both publisher notices, fully verifies destination lengths/SHA-256 pins,
synchronizes files/directories/ancestor entries and atomically publishes one
complete generation. Reimport creates new files; old generations are retained.
Writer exclusion uses a persistent standard-library advisory file lock. Failure
before manifest replacement preserves the old selection; failed synchronization
after replacement returns a distinct uncertain-durability error.

Normal dictionary-backed CLI commands use `$HOME/.yomibu/dictionaries` or explicit
`--dictionary-dir`. Startup checks bounded records, compiled pins, private OS
ownership/permissions, regular non-symlink/non-hardlinked files, exact sizes and
the system header/`20260723` description, then maps the same checked handle.
Explicit `--dictionary PATH` always performs full verification into owned bytes.
The policies share one embedded-configuration constructor, tokenizer and evaluator.
One analyzer is reused throughout each command. Generation preflight/settings,
preview independence and offline analysis remain intact.

The mapping contract is conditional: actual full installation verification and
unchanged dictionary bytes for the analyzer lifetime. Receipts are not signatures;
metadata, read-only modes and Linux/macOS advisory locks cannot prove immutability.
Arbitrary external writes/truncation can invalidate mmap-backed access. This does
not retain the owned-buffer guarantee against such writes. The public managed
constructor is `unsafe` so Rust callers must uphold these obligations explicitly.
Managed provenance reports installation-time verification, startup checks and
file stability; its checksum identifies the expected pin. Owned JSON keeps its
existing serialized shape. See [dictionary usage/safety](docs/DICTIONARY.md).

Direct dependencies on already-locked `memmap2 0.9.11` and `libc 0.2.189` permit
mapping the checked handle and performing effective-UID/no-follow/nonblocking
checks. The lockfile review confirms only two root dependency edges were added:
every other package/version/checksum/dependency record is identical. No analyzer,
dictionary, embedded configuration, feature, provider, runner or caching upgrade.

### Observed Red–Green–Refactor

| Cycle | Observed RED | GREEN and explicit refactor review |
| --- | --- | --- |
| Verified installation boundary | Integration tests could not import the missing managed-installation API. | Copy/verify complete pinned bundles and publish separate generations; arbitrary directories fail. Reviewed concrete receipt/selection shapes and kept the developer installer separate. |
| Writer coordination and durability | A second importer published while a lock was held; final sync failure reported an ordinary I/O failure. | RAII writer reservation and distinct post-publication uncertainty; tiny real-filesystem fault tests pass. Reviewed guard lifetime, descriptor inheritance, retained bundles and error meaning. |
| Managed checks | Wrong headers, symlinked paths, writable files/shared roots were accepted. | Same-handle header/length checks and private ownership/type/no-follow checks; all regressions pass. Reviewed the limits of permissions/receipts instead of claiming immutability. |
| Mapping and provenance | Equivalence test could not call the absent managed loader. | Map the checked handle, share embedded construction, report installation verification. Tokenization/evaluation agree across policies; old mapped readers survive publication, and owned readers survive source truncation. Reviewed storage lifetime, borrowing and report compatibility. |
| Notices and CLI | Missing notices were accepted at startup; executable tests rejected the missing dictionary command and managed options. | Cheap notice checks, explicit offline import/verify, shared CLI source selection and truthful reports. All new executable tests pass. Reviewed safe diagnostics, HOME/preflight ordering and common rendering. |
| Ancestor durability and retry | Injected ancestor sync failure did not prevent first publication; retry with already-created directories also bypassed this boundary. | Synchronize ancestor entries even after interrupted setup; pre-publication failure tests pass. Reviewed and simplified retry logic, without inferring durability from metadata. |

Private filesystem checkpoints are test infrastructure, with no runtime switches.
Process-kill tests cover copy, file sync, bundle/manifest publication and final
sync boundaries, complete visible bundles and lock release. They do not simulate
power loss. Further coverage of existing behavior required no artificial RED:
both generation commands use local mock HTTP services; focused generation sends
the exact preserved v2 request fixture. Existing executable-boundary tests remain.

Final REFACTOR review covered production/tests for simplification, duplication,
naming, modelling, ownership/borrowing and idiomatic Rust. Kept one concrete
adapter and common storage constructor/CLI loader; no analyzer trait or service
hierarchy was justified. Removed the magic suffix slice in favor of checked
prefix removal. Subsequent focused tests and the final full suite passed.

### Verification and preservation

Pinned Rust/Cargo 1.98.1 on Linux/x86_64 passed:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all`: **217 passed**, none failed/ignored, including two
  doctests. The full suite passed before and after the final ancestor retry fix.
- `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`
- Nine existing offline Python installer tests and documented setup, which fully
  verified the existing dictionary/notices without downloading.
- Existing preview CLI/library, prepare library and A1 smoke demonstrations, plus
  managed import/verify/analyze demonstrations. Synthetic smoke is not scored A1.
- Whitespace review and intentional lockfile review; locked checks leave the
  reviewed lockfile byte-identical. The existing CI lockfile/whitespace gates remain.

An inventory/hash audit preserves all 39 protected tracked files: frozen A1
documents/fixtures, comparison manifest/requests/recipe, embedded configuration,
provider adapter code, both Python installer files, toolchain and CI workflow.
Comparison code pins `818dda5e6897e4d8ab729ed9198e5070d16af50b` and
`6c5e1343ca6583fc76211c6a757ec0070f5e7325` remain unchanged. No scored holdout,
private reference material, paid provider call or merge was involved. Native
macOS and hosted CI were not run for this implementation; existing matrix/gates
are unchanged.

### Before/after release execution measurements

The exploratory Linux probe observed owned/full-hash loading at 260–360 ms and
about 237 MiB peak RSS, versus managed mapped loading at 24–67 ms and about 56 MiB;
mapped/full-hash loading still reached about 237 MiB. Its roughly 207 MiB anonymous
memory reduction is exploratory Linux evidence, not macOS results or a guarantee
for larger workloads.

Built the unmodified main release executable before application edits, separately
from execution. Ten initial warm-cache baseline processes had median wall time
216.3 ms and identical output hashes. Then prebuilt the final candidate release
executable. Build durations are not a controlled compilation comparison: builds
overlapped unrelated verification work. No compilation improvement is claimed.

Final measurements used the same Linux/x86_64 machine, Rust/Cargo 1.98.1, pinned
dictionary, embedded configuration and public `tests/fixtures/analyze/nominal.json`.
Both executables and dictionary files were on overlayfs; the baseline copy was
byte-identical to the original before-edit executable. No competing build/test
ran during these measurements. One unmeasured warm-up per case preceded ten
fresh processes/case, ordered baseline → external → managed for each repetition.
Filesystem cache was warm; no controlled cold-cache experiment was performed.

```sh
# Direct prebuilt executions, no cargo/compiler during measurements:
target/dictionary-startup-2026-10-05/baseline-yomibu analyze \
  --dictionary /workspace/yomibu/target/a1/current/system_core.dic \
  --input /workspace/yomibu/tests/fixtures/analyze/nominal.json --json
target/release/yomibu analyze \
  --dictionary /workspace/yomibu/target/a1/current/system_core.dic \
  --input /workspace/yomibu/tests/fixtures/analyze/nominal.json --json
target/release/yomibu analyze \
  --dictionary-dir /workspace/yomibu-managed/target/dictionary-startup-2026-10-05/managed \
  --input /workspace/yomibu/tests/fixtures/analyze/nominal.json --json
```

| Final overlayfs executions, ten/case | Wall median (range), ms | Peak RSS median (range), MiB |
| --- | ---: | ---: |
| Baseline owned/full SHA | 232.0 (224.9–266.2) | 240.1 (239.9–240.3) |
| Candidate external owned/full SHA | 234.5 (218.7–249.1) | 238.7 (238.7–238.8) |
| Candidate managed mapping | 17.9 (16.7–24.4) | 62.1 (62.0–62.3) |

Wall time includes process launch, initialization, analysis and JSON output to a
file. Linux `wait4` supplies per-process user/system time and kernel peak RSS in
KiB, converted to MiB; compilation and import/verification are excluded. All 30
executions succeeded. External reports are byte-identical to baseline; managed
reports match after removing only the explicit loading-provenance field.

Separate diagnostic processes (five/case) sampled `/proc/PID/smaps_rollup`, with
a requested 1 ms sleep plus sampling overhead. Median sampled maximum anonymous
RSS was **234.8 / 234.1 / 27.5 MiB** for baseline/external/managed. Corresponding
non-anonymous resident pages (`Rss - Anonymous`) were **5.3 / 4.0 / 29.3 MiB**;
file PSS maxima were **4.9 / 3.5 / 28.8 MiB**, with zero shared-memory PSS in the
final runs. These samples may miss brief peaks; maxima are separate observations
and must not be summed or substituted for kernel peak RSS.

An earlier measurement round placed managed files/baseline executable on tmpfs;
Linux classified those mapped pages as shared memory. Those observations are
retained separately rather than mixed into the final file-backed comparison.
The final results show about 92% lower median CLI wall time and roughly 207 MiB
less anonymous residency for this short warm-cache workload. They are not
whole-host memory savings, macOS measurements, cold-cache results or runtime/
memory guarantees for larger workloads. File-backed pages can be shared/reclaimed;
full hashing still touches every page.

Temporary measurement drivers are outside Git; no persistent benchmark framework
or timing-threshold tests were added. Raw logs/results and the copied baseline
are retained under ignored `target/dictionary-startup-2026-10-05/` and
`/tmp/yomibu-managed-measurements/`.

Rust notes for a Ruby developer: one upstream storage enum retains either owned
bytes or a mapping. Borrowed shared analyzer references reuse it without copying
217 MB. RAII scopes the writer lock; typed errors preserve whether publication
happened. The managed constructor's `unsafe` contract makes external file-stability
obligations explicit instead of suggesting the borrow checker proves them.
