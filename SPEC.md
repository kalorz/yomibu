# Yomibu specification

This is the authoritative product and architecture baseline, accepted on
2026-09-27, with vocabulary clarified on 2026-09-30 and the existing-use-case
architecture migration authorized on 2026-10-01, followed by the generation
design and Cloud handoff on the same date, and bounded A1 evaluation scope
accepted on 2026-10-03, with A1 implementation subsequently approved that day,
and a separate offline analysis CLI and G1 experimental candidates authorized on
2026-10-04, G2 focused experimental context on 2026-10-05, and the shared
learner-inventory story path on 2026-10-06. Changes must
be reflected here, in [ARCHITECTURE.md](ARCHITECTURE.md), and in `PLAN.md`.
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

Do not define a permanent meaning of "known" in milestone 1. Preserve the source
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

Milestone 1 contains no LLM integration, generation, quiz, RAG implementation,
embeddings, vector database, PostgreSQL, Bunpro, JMdict, or web interface, including
placeholder versions. Local files are sufficient for the CLI proof of concept.
PostgreSQL is the likely later web database; pgvector remains conditional on a
real vector-search need.

## Retired manual preview and source preparation — 2026-10-06

The G0 `preview` and source-inspection `prepare` commands, their examples and
library entry points (`preview`, `preparation`, `adapters::grammar_file`) are
retired without aliases. [COMMAND HISTORY](docs/COMMAND_HISTORY.md) preserves
purpose, original contracts and the last complete Git pin. Current preview and
retrieval are `preview-story` and `prepare-retrieval`.

Eligibility remains active: `LearnerKnowledgePolicy::derive(source)` validates
source data and returns ordered decisions with original evidence, without grammar
input or persistence. `LearnerKnowledge` no longer carries grammar declarations.
Inventory projection uses these unchanged rules. Grammar descriptions/bindings
remain separate inputs to story assessment and supplied-text analysis.

## A1 — bounded offline analysis evaluation (complete; no-go)

Before generation, investigate supplied modern Japanese sentences of at most
100 Unicode characters: vocabulary identity, regular godan/ichidan polite
present/past/negative forms, narrowly scoped topic は and object を, and nominal
です. Preserve compounds and components; familiar components or kanji cannot
authorize a whole word. Other grammar, multiword expressions, and reading/sense
ambiguity expose limitations. No general grammar or contextual-sense validator,
generation, repair, quiz, provider integration, or speculative framework.

Keep completed `Pass`/`Fail`/`Inconclusive` judgments distinct from execution
errors and `NotRun`. No A1 report may describe sentences as accepted exercises.
This historical A1 slice preserved the then-current preview/preparation contracts. A1
evaluation uses explicit bindings while preserving free-form grammar inputs;
it must not automatically interpret learner declarations.

The user approved manually operated blind review by another AI provider instead
of qualified human reviewers. Model-derived judgments remain provisional;
verify citations and retain disagreements and alternatives. Model agreement is
not independent linguistic ground truth. Learner material never enters a review
packet or external service. Only original, explicitly reusable synthetic fixtures
may be published, with held-out cases private until scoring.

The [reference protocol](docs/A1_REFERENCE_PROTOCOL.md),
[packet format](docs/A1_REVIEW_PACKET.md), and
[manual review prompt](docs/A1_EXTERNAL_REVIEW_PROMPT.md) define the source-backed,
provisional evidence process. Implementation was explicitly approved after the
protocol-only stage. The source gate requires fully source-backed outcomes for
all 48 core cases before freezing; model-only/disputed cases cannot establish
the accepted held-out targets. The 60-case plan and thresholds are unchanged.
Before freezing, a source-limited family may be replaced with a retained
exclusion/revision log; preserve the original evidence and family separation.
Revision 2 applies this to two complete development pairs, without changing
scope or awarding new references automatic acceptance.

The concrete implementation uses Sudachi.rs v0.6.11 pinned to Git revision
`90fd6068c80c2fc3b63e0dbab0e341475bad4d8f` and checksum-verified SudachiDict Core
20260723 V0. Built-in configuration disables normalization/path rewrites and uses
only the fixed SimpleOovPlugin with embedded character definitions. The dictionary
is explicitly loaded into owned memory; there are no ambient files or runtime
downloads. C/A units and original spans remain in the report. Exact artifact pins,
setup/license notices, and verification are in [A1 implementation](docs/A1_IMPLEMENTATION.md).

Explicit setup verifies size and SHA-256 for the dictionary and both notices,
then publishes a complete bundle through `target/a1/current` in one atomic link
replacement. It verifies all three files before cache reuse. Completed bundles
remain available to existing readers; pre-publication failure preserves the prior
bundle, while failed directory synchronization after publication reports uncertain
durability. Legacy flat files are retained; current examples use
`target/a1/current/system_core.dic`. This setup hardening does not change the pinned
analyzer/dictionary, frozen references, or completed A1 results.

`analysis::Sentence` validates nonblank input and counts at most 100 Unicode scalar
values. `evaluation::evaluate` is synchronous and takes concrete analysis, unchanged
free-form declarations, and explicit word/rule bindings. Vocabulary tuples retain
written form, reading, sense, and optional-use evidence represented by the explicit
`direct_object` boolean. False means no positive object-use binding, not a claim
of intransitivity. This metadata requires source review; it is not inferred from を.
A sense label does not establish contextual correctness. Unknown identities,
competing supplied tuples, and reading mismatches remain Inconclusive.
Reference review verifies that the supplied sense is attested, while contextual
sense selection stays unassessed. Ordinary polysemy alone does not invalidate
that bounded check. Unresolved lexical/POS alternatives affecting a required
check remain Inconclusive; dictionary entry grouping cannot settle them. The
reference protocol records this clarification and its version history.

Recognition is limited to the two single-clause patterns and five checks in the
[common binding sheet](docs/A1_BLIND_PACKET_HEADER.md): optional nominal topic with
nominal です, or optional topic/direct object with a regular polite verb. At most
one final 。 is optional; other punctuation, whitespace, extra clauses/modifiers,
questions, and unsupported morphology expose scope limits. A nominal slot is one
whole lexical unit, possibly a compound. Each of the seven rule variants requires
an explicit binding to an existing declaration ID; descriptions are never parsed.
Malformed bindings/analysis are typed errors. All original text must be accounted
for before Scope can Pass. Required check applicability remains unresolved for
an unrecognized construction. Supported permission violations keep their Fail
reason/span even when other checks are Inconclusive.
Scope does not mirror permission results: a missing permission belongs to its
applicable check, while Scope independently assesses structural coverage and
applicability. The [Scope clarification](docs/A1_SCOPE_CLARIFICATION.md) records
this existing v0.3 interpretation for reference reconciliation before freeze.

Every report lists naturalness, multiword expressions, and contextual reading/sense
as unassessed. In the frozen A1 implementation, a literal-looking idiom could pass
bounded structural checks; three such challenge results blocked the protocol's
go decision. The separately recorded follow-up below restricts object-combination
coverage. There is no ad hoc phrase blacklist or general validator.

The A1 synthetic packet runner is retired as of 2026-10-06. Its historical
1 MiB/60-case format and executable remain reproducible at revision
`d2adfcfe6dffede63363bf1a11be81ce8fe85606`; [A1 evidence](docs/A1_IMPLEMENTATION.md)
retains setup, inputs, conclusions and frozen scoring records. Current story,
evaluation and analyzer tests preserve completed judgments, execution errors,
NotRun, original text/spans and partial results. Packet-only format tests retired
with the runner; they are not a current product contract.
The ordinary CLI, schema 1, and earlier use cases retain their existing contracts.
No generation, repair, quiz, provider calls, or automatic interpretation of learner
grammar is authorized by this milestone.

Contract tests and a three-case smoke demonstration provide engineering evidence.
The original, explicitly reusable 24-development/12-challenge set is now frozen
and evaluated against provisional source-backed references. The separate
custodian reports all 24 held-out references reviewed, reconciled and frozen,
meeting the reported full reference gate. Previously exposed held-out families
were replaced with history retained. Development outcome and negative reason/span
checks match after a small
reporting fix. Three unsupported challenge Pass results prevent a bounded go.
After hash verification, the released input ran once offline using the frozen
executable: all 24 cases and 120 checks completed without execution errors.
The custodian's scoring receipt reports all held-out targets met: 24/24 outcome
and 120/120 check judgments match, with 11/12 exact negative reason/span matches.
The remaining nested-span discrepancy is preserved without revising the reference.
The investigation is complete with a no-go because the challenge safeguard failed.
No hidden reference labels were opened here, and no change or rerun was made for
that scored investigation. Held-out fixtures remain private absent a separate
release decision. See [evaluation status](docs/A1_EVALUATION_STATUS.md); no linguistic
acceptance or exercise-acceptance claim follows.

### Separate object-combination safeguard — 2026-10-04

The authorized follow-up starts from local A1 checkpoint `f1237d4`. It uses only
visible development material and leaves the completed evaluation, frozen references,
and held-out discrepancy intact. It does not rerun the scored holdout.

A transitive-use binding describes one vocabulary tuple; it cannot resolve an
object/predicate combination. For a recognized object + を + regular polite verb,
Particles and Scope therefore retain Inconclusive when permissions are present,
even with `direct_object: true`. Existing permission failures still determine Fail
and retain their decisive spans; the unresolved combination is reported alongside
them. Its finding spans the object through the predicate, excluding a preceding
topic and the optional final 。. Per-word vocabulary and morphological checks
remain separately observable.

This deliberately reduces coverage for ordinary object sentences as well as the
three exposed multiword cases. It does not identify idioms or supply contextual
evidence. No phrase exceptions, new bindings, dependencies, or broader grammar
support are introduced. See the [follow-up record](docs/A1_FOLLOWUP.md) for TDD,
visible-only results, verification and limitations; none revises the A1 no-go.

## Offline analysis CLI — separate milestone, 2026-10-04

```sh
yomibu analyze --dictionary PATH --input PATH [--json]
```

This command exposes the existing bounded analyzer and evaluator for one manually
supplied sentence. It adds no linguistic rules or acceptance decision. A1 remains
complete with its historical no-go, 24/24 held-out outcomes, 120/120 check judgments,
and 11/12 exact negative reason/span agreement. The separate object-combination
safeguard and its reduced ordinary-sentence coverage remain in force. Neither the
scored holdout nor frozen references are development material for this milestone.

Input is one UTF-8 JSON object with exactly these required fields:
`{"version":1,"sentence":"犬です。","grammar":["です"],"bindings":{"vocabulary":[{"written_form":"犬","reading":"イヌ","sense":"dog","direct_object":false}],"grammar":[{"declaration_id":1,"rule":"NominalDesu"}]}}`.
Reads stop at 64 KiB + one byte; reject documents exceeding 65,536 bytes before
decoding. Reject malformed JSON/UTF-8, unsupported versions, unknown or missing
fields, blank declarations, and invalid bindings. The unchanged `Sentence` limit
is nonblank text of at most 100 Unicode scalar values, with no normalization or
trimming. This is a supplied text bound, not a new sentence-segmentation validator.

`grammar` preserves free-form descriptions, order and duplicates; IDs are their
one-based positions. Bindings reuse `EvaluationBindings` exactly: nonblank whole
word/katakana-reading/sense tuples, explicit `direct_object` booleans, and the seven
existing `GrammarRule` names. Empty arrays are valid and grant no permissions.
Descriptions never grant rules implicitly; sense and transitive-use declarations
are user assertions, not verified contextual evidence. No research reuse statement,
case ID, reference label, or learner cache is required. The A1 synthetic harness
retains its separate packet format and reuse contract unchanged.

Text output names the overall completed outcome and all five required checks,
coverage, every finding's reason, half-open original UTF-8 byte range, and quoted
affected text. It requires no terminal color. Sentence/declaration/excerpt content
is escaped for terminal display. JSON output has `version: 1`, exact decoded
`input`, complete C/A `analysis` with analyzer/dictionary/configuration provenance,
`outcome`, and the unchanged `evaluation` including all limitations and findings.
Escaping changes presentation only; decoded strings and byte offsets remain original.
Naturalness, multiword expressions, and contextual reading/sense remain explicitly
unassessed. A Pass never establishes an accepted exercise.

Completed Pass, Fail and Inconclusive all exit zero. Invalid inputs or execution
errors exit nonzero with an escaped, contextual stderr diagnostic and no evaluation
on stdout. They are never converted to linguistic Fail or completed checks.
Argument diagnostics preserve line breaks and indentation while escaping supplied
values; usage and help use the fixed executable name `yomibu`.
There is no structured error-report contract in this version.

Explicit `--dictionary PATH` retains the fully verified owned adapter policy and
needs no HOME. Without it, the command selects the managed installation described
below; `--dictionary-dir PATH` overrides its HOME default. Missing/unpinned files
or invalid managed installations fail without fallback. Explicit development
setup remains separate at `target/a1/current/system_core.dic`.
The command ignores `--data-dir` and performs no learner access, credential lookup, sync, network,
download, persistence, telemetry, or async runtime startup. Argument/file/output
work belongs to the executable; library analysis/evaluation remain independently
callable. See README for runnable synthetic examples and PLAN for TDD/check evidence.

## Shared learner-inventory story path — 2026-10-06

This approved breaking slice is the only current generation path; G1/G2
implementations and old preview/preparation are retired below. Sync/status and
analyze contracts remain unchanged. Full current formats/limits are in
[STORY GENERATION](docs/STORY_GENERATION.md); measured retrieval evidence is in
[RETRIEVAL](docs/RETRIEVAL.md).

`LearnerInventory` is the source-independent complete material for an attempt.
Project the existing explicit WaniKani eligibility policy or a versioned manual
inventory into it; manual grammar/vocabulary may supplement WaniKani. Preserve
original source state, source-qualified IDs, accepted-answer reading/meaning
alternatives, missing kana readings and unknown direct-object evidence. Never
construct reading/sense Cartesian pairs or infer transitivity from source POS.
Normalize kana only for analyzer comparison. Source eligibility is not mastery.

`StoryRequest { version, brief, targets: { vocabulary, grammar } }` uses stable
string IDs, zero or more targets and a free-form topic/scene. Targets must exist
in the complete inventory. They do not grant material or create grammar rules.
`StoryGenerationOptions` separately controls execution: `candidate_count` defaults
to 2 and is exposed as `--candidates N` in generation and preview. Counts must
be positive and fit checked `512 × N` output-token arithmetic, with no arbitrary
4/8 ceiling. Real provider token limits and the 64 KiB response cap still apply;
large requests may fail explicitly, without clamping, splitting or retry. Future
repair settings belong here only when repair exists. Story-request JSON keeps
brief/targets, not execution choices.
`StoryVocabularySelection` records the prompt subset: every vocabulary target,
then supports ranked by brief similarity, with ID-based ties. The canonical library
entry pair is `story::plan_generation` followed by `story::generate_story`, adjacent
in `story.rs`. Offline planning validates inputs/options/selection bounds, selects
once, prepares bounded request bytes and projects full-inventory assessment inputs.
It returns an immutable `StoryGenerationPlan` containing the final selection,
`AiModelRequest` and `StoryAssessmentInputs`. The outgoing request owns only
bytes/hash and encoded options; assessment borrows the original inventory/request
and selected IDs, with its own structural-check projection.

`generate_story` receives this plan, an explicit concrete client and analyzer.
It makes one provider attempt, assesses every original candidate and returns an
owned `StoryGenerationResult`. Provider failure returns `ProviderError`; candidate
execution errors remain beside partial/completed assessments in the result.
Results outlive input data and execution resources. The complete inventory remains
the assessment boundary; no generic pipeline or separate constraints/context file
is introduced. Lower-level selection/request/assessment functions remain available.

Rust API changes: the former subset-only `StoryGenerationPlan` is renamed
`StoryVocabularySelection`; `StoryGenerationPlan` now represents complete preflight.
`Sentence` supports borrowed or owned text, loses `Copy`, and `text` borrows `&self`.
Ordinary analysis still borrows; returned story analyses own a bounded text copy
without changing original spans or serialized reports. No wire-format change occurs.

CLI `commands::story::generate` loads inputs/embeddings, calls library planning,
initializes dictionary/credentials/runtime, awaits library generation, then renders
and determines exit status. Library orchestration owns no environment lookup,
file selection, runtime or terminal. An API caller uses the same entry pair.
Preview is a separate offline operation with no dictionary, credentials, HTTP
client or runtime and no cache refresh. Missing/stale embeddings fail explicitly.
`prepare-retrieval` may prepare vectors; generation may do so only with explicit
backend configuration. Hosted work additionally requires `--allow-embedding-call`.
`--allow-model-call` independently authorizes the generation attempt.

Shared serializable report projections live in `reports::{story,candidate,analysis}`.
Candidate execution-error classification and NotRun check data belong to the
library; terminal-safe JSON/text rendering belongs to CLI `output/`. Report
construction does not rerun analysis or assessment and leaves serialized keys,
original texts/spans, provider request bytes and hashes unchanged.
`adapters::input_file::read_bounded` takes an explicit path and caller-supplied
limit; it has typed errors and performs no environment lookup or writes.
CLI `args.rs` defines arguments, `commands/` selects resources and invokes library
use cases, and `main.rs` handles parsing/exit status. No generation app wrapper,
service hierarchy, API placeholder or new dependency is introduced.

Reusable retrieval preparation is `retrieval::prepare_cache`: reuse compatible
vectors, encode missing inputs in groups of 32 and validate the complete result.
`adapters::embedding_cache_file::EmbeddingCacheFile` owns bounded reads and atomic
publication with typed pre-replacement/uncertain-durability errors. The host selects
path, encoder, credentials and executor; no file is replaced after a batch failure.

Use one narrow `Embedder` port for local/hosted encoders, a model-specific flat
vector cache and cosine similarity. Identity includes provider/model/revision,
dimensions and encoding revision. Invalid vectors/partial responses fail; cached
files replace atomically only after successful complete preparation. Validate all
combined lexical documents against the 32 KiB per-input limit before provider
work. HTTP batches also respect actual encoded JSON size (512 KiB), including
escaping, and preserve input order across split requests without retry. Embedding
requests contain lexical documents/brief, without learner IDs/progress history.
Generation sends only selected vocabulary, brief/targets and supplied grammar.
No implicit remote call, automatic model selection or fallback occurs. The explicit
lexical baseline is nonsemantic; a local/hosted quality comparison must precede
choosing a default dense model. Those two comparisons remain blocked on configured
model services/credentials; this is not a completed model-selection claim.

Generation uses OpenAI Responses `gpt-6-luna`, Standard/default tier, reasoning
`none`, `512 × candidate_count` output tokens, no tools, streaming or background
work, `store: false`, truncation disabled and explicit prompt caching without
breakpoints. Disable redirects, environment/system proxies and protocol retries.
Connect timeout is 5 seconds; the 30-second request deadline includes body reads.
Responses are bounded to 65,536 bytes, checking declared size and every chunk.
Require a completed envelope and exactly one completed assistant text payload;
documented reasoning items are ignored metadata. Refusal, errors/incompleteness,
unknown outputs/tool calls, missing required fields, invalid JSON/UTF-8, duplicate
payload fields, wrong counts/types or extra payload fields reject the whole
response without salvaging a candidate. Optional usage/tier/request ID may be
absent. Provider identity and usage are claims, not immutable pins or billing
verification. Protect credentials; errors never reflect response bodies or keys.

The current slice produces the requested number of experimental single-sentence candidates,
each nonblank and at most 100 Unicode scalar values. New prompt revision:
`story-inventory-v1`; request cap remains 16,384 bytes. Freeze the final prepared
body/hash and send unchanged, with count-specific schema and output-token budget; other provider settings,
limits and one-attempt/no-retry behavior remain unchanged. The parser requires
exactly the requested count. Request bounds precede dictionary/generation-credential startup.
Explicit targets and grammar are never silently removed for size; optional
supports may be removed during preparation without another selection pass.

Keep all original texts and all available analysis after candidate execution
failure. Distinguish Fail, Inconclusive, execution error and NotRun. Multiple
vocabulary/grammar target observations and unselected-inventory departures remain
separate from bounded checks. Alternatives, competing identities, missing evidence
and unsupported morphology cannot become positive intended-use claims. Preserve
partial completeness beside an observed target with unrelated uncertainty.
Object grammar observations require the same unambiguous direct-object lexical
evidence as the structural checker; unresolved evidence leaves the construction
unassessable, while the object-combination safeguard remains Inconclusive even
when a bounded occurrence is observed.
A1's no-go, original UTF-8 spans and object-combination safeguard remain unchanged.
Topic adherence, contextual reading/sense, naturalness and comprehension remain
unassessed; no accepted exercise, longer-text generation, repair or live generation
evaluation is part of implementation verification. "Story" includes single-sentence
scenes/dialogues and adds no narrative-content requirement.

CLI names are `prepare-retrieval`, `preview-story`, `generate-story` with
`--inventory`/`--wanikani-cache`, `--request` and `--embedding-cache`. Removed
`generate-candidates`, `generate-focused`, `context-preview`, `--permissions` and numeric `--focus-entry`
have no aliases. `preview-reading`/`generate-reading` and the `reading` Rust module
are also replaced without aliases. Current Rust/JSON vocabulary agrees; current hashes and schemas
intentionally changed during the story migration. Retired implementations and
exact-byte fixtures remain reproducible from the historical code pins and archive;
they are not current library APIs. JSON/text keep terminal escaping, diagnostic
layout, all candidate results and the existing exit-status distinctions.

## Retired G1/G2 generation — 2026-10-06

The story path is the only current generation path. `generate-candidates`, the
G1/G2 library entry points, authored situation selector and focused observation
APIs are removed without aliases. Historical inputs, exact request snapshots,
comparison manifests and code revisions are archived in
[GENERATION HISTORY](docs/GENERATION_HISTORY.md). Their engineering evidence does
not establish reading quality or accepted exercises; the G2 v1/v2 live comparison
remains **not run**. A1's no-go and object-combination safeguard remain unchanged.

Current story request/report bytes, hashes, settings, limits and assessment behavior
are unchanged by retirement. `Client::generate_story_candidates` now returns
`ProviderError` directly instead of the redundant `GenerationError::Provider`
wrapper. `GenerationProvenance` loses the historical `focused_context` field,
which was absent from current story JSON. Shared candidate assessment/error and
provider metadata types remain in `generation`; no archived Rust implementation
is compiled or exposed.

## Managed dictionary loading — 2026-10-05

Normal analyze/generate-story commands select a managed
installation when `--dictionary` is absent. The default is
`$HOME/.yomibu/dictionaries`; `--dictionary-dir PATH` overrides it and conflicts
with `--dictionary PATH`. Missing HOME requires an explicit managed root.
Input/preflight validation precedes resource loading; model opt-in, credential
ordering and provider settings stay unchanged. Preview commands remain independent.

`yomibu dictionary import --bundle PATH [--dictionary-dir PATH]` copies the exact
dictionary and both publisher notices, fully verifies the staged destination
lengths/SHA-256 pins, synchronizes them, and atomically publishes a new generation.
`dictionary verify` fully verifies the selected bundle without modifying it.
There are no implicit downloads, installs, repairs or fallbacks. The existing
Python developer/CI archive installer remains separate and unchanged.

Completed generations are never modified/truncated in place or automatically
removed. Updates switch one bounded versioned manifest after publishing the
complete bundle. Existing readers retain the old mapped file. Pre-publication
failure preserves the previous selection; post-publication directory-sync failure
reports uncertain durability with the complete new selection visible.

Managed startup checks private OS ownership/permissions, regular files without
symlinks/hardlinks, bounded installation records, exact sizes and the dictionary
header format/description, then maps the same checked handle. Header format and
description are compatibility checks, not exact content verification. Installation
receipts record earlier full verification; they are not signed proof of present
contents. Controlled import/update behavior supplies a conditional stability
contract: no actor may modify the mapped dictionary during an analyzer's lifetime.
Advisory locks, metadata and read-only permissions cannot prove immutability or
preserve the owned-buffer guarantee against arbitrary external writes. Use the
owned policy where that contract cannot be maintained.

One concrete SudachiAnalyzer and embedded configuration serve both policies.
An analyzer is reused per command/session, without assigning policy by process
lifetime. Managed provenance adds installation verification, startup-check and
file-stability information; its dictionary checksum remains the expected pin.
Owned reports retain their serialized shape, and CI/reproducibility experiments
retain full verification. Exact analyzer/dictionary/configuration pins, evaluator
behavior, frozen A1 evidence and v1/v2 artifacts/code pins remain unchanged.
See [installation and safety](docs/DICTIONARY.md).

## Design vocabulary and composition

This vocabulary records the accepted direction. `WaniKaniSyncData`, `App`,
`LearningSource`, `LearningStore`, `SourceSyncWriter`, and the file/in-memory
stores exist for single-account sync/status. `LearnerKnowledgePolicy` and
`LearnerKnowledge` implement the concrete WaniKani eligibility projection
slice. The remaining names do not authorize placeholder types, traits, or future
product features.

| Name | Responsibility |
| --- | --- |
| `WaniKaniSyncData` | Holds normalized account data, progress, associated accessible material, and exclusions collected during one WaniKani synchronization interval. |
| `LearningSource` | Retrieves normalized data for the current WaniKani synchronization slice without persisting it; generalization awaits another real integration. |
| `LearningStore` | Reads a coherent source-data version and reserves an exclusive writer in the current physical account-scoped store. |
| `SourceSyncWriter` | Owns one synchronization reservation and validates/publishes its related material and progress together. |
| `SourceConnection` | Identifies one learner's connection to a particular provider account or input, separately from the provider kind and the Yomibu learner identity. |
| `LearnerProgress` | Preserves source progress and manual learner declarations, with their provenance, without fixing a definition of mastery. |
| `LearnerKnowledge` | Describes the material treated as known after applying the chosen rules to selected learner progress. |
| `LearnerKnowledgePolicy` | Deterministically derives learner knowledge from progress without fetching sources, persisting results, or inventing cross-provider grammar equivalences. |
| `LanguageMaterialStore` | Provides stored language material independently of an individual learner's progress, subject to the caller's access rights. |
| `LearnerProgressStore` | Provides learner progress within the explicitly selected learner and source-connection scope; synchronized updates publish through the shared writer boundary. |
| `ExampleSearch` | Retrieves usage examples matching explicit search criteria. |
| `PromptTemplate` | Describes reusable prompt content with inputs to be supplied for a particular model task. |
| `PromptStore` | Provides identified, versioned sets of prompt templates. |
| `AiModelRequest` | Owns the finalized serialized payload, hash and encoded execution options for one AI model invocation; assessment inputs remain separate. |
| `LanguageModel` | Executes a prepared model request using the explicitly selected model backend. |
| `CandidateGenerator` | Produces exercise candidates that still require validation. |
| `PromptedCandidateGenerator` | Uses a prompt store and language model to produce candidates for the defined generation task. |
| `ExerciseGenerator` | Prepares context, obtains candidates, validates them, and selects or repairs within the budget, returning an accepted exercise result or a typed failure. |
| `App` | Coordinates library use cases, including explicit synchronization, learner-data loading, knowledge preparation, and exercise generation. |

Use these responsibility names consistently rather than interchangeable
`Repository`, `Storage`, `Service`, `Runner`, or `Executor` suffixes. `Store`
identifies stored-data access; each contract must still specify write support,
durability, consistency, and failure outcomes. `Search` names retrieval by
criteria, `Client` communication with a particular external service, and `Policy`
deterministic decision rules. Not every responsibility needs a trait or a separate
struct. Add an extension point when an actual caller needs to substitute it.

Use `InMemory...`, not `Memory...`, for an adapter backed by process memory.
Use backend or provider names for other adapters, such as `FilePromptStore` and
`PostgresPromptStore`. Private data does not require a `Private...` adapter.
Keep `LearnerKnowledgePolicy`; use `ExerciseGenerator` instead of
`GenerationEngine`. Stored prompt templates and prepared model requests have
different contracts and must not both be called `Prompt`.

The future generation model distinguishes an immutable candidate, its text
structure, analysis tied to that candidate, check outcomes, and the accepted
exercise. `Story -> Paragraph -> Sentence` is the logical text hierarchy; tokens
and morphological/grammar hypotheses belong to analysis. Findings identify the
candidate and their exact text/analysis scope. Repair creates a new candidate;
initially reanalyze and reassess it in full. Do not reuse old findings as evidence
about edited text. Concrete representations and public exports follow actual
use cases, not every node in a conceptual diagram.

Generation is an extension point alongside analysis and checking. Components
may cover one concern across several phases through small role-specific
contracts; no umbrella plugin trait, manifest, dependency graph, discovery
system, or new crate is required. Trusted in-process components follow explicit
I/O boundaries; Rust traits do not provide a sandbox.

Typed model-task inputs/outputs coexist with replaceable, versioned prompt
content. Compose compatible contributions into at most one model invocation
per execution of the generation, review, or repair phase. Also bound the total
invocations, repair iterations, and candidate count across the operation, with
explicit deadlines and token/cost limits where supported. Retries count toward
the total and must not be hidden in adapters; a phase must not silently split
into multiple invocations. Incompatible contributions or an oversized request
produce an explicit limitation. A parsed typed response is not proof of its
linguistic correctness, and checks combined in one call are not independent
judges. No automatic retry follows uncertain model completion.

A source is a provider or input of material/progress; a connection selects a
specific account or input for a learner. An observation is a recorded source fact,
not proof of mastery and not necessarily an event in an append-only history.
The current `domain::Learner` contains WaniKani account information; its ID must
not become the identity of a future multi-source Yomibu learner. Keep application
settings, learner data/source selections, and individual exercise requests
separate instead of combining them in `ProfileConfiguration`.

### Data flow and storage boundaries

The composition root in the CLI or Cloud reads settings and resolves secret
references, opens the required resources, and constructs dependencies. Creating
`App` or a generator does not synchronize sources, start background work, or
generate an exercise. Resource loading/connection has explicit operations.
Future multi-learner calls supply the learner/input; shared objects have no mutable
current user. The current sync/status API uses the store as an explicit single
WaniKani account scope, refusing replacement by another account.

The intended flow is arguments/request -> settings and secret resolution ->
dependency construction -> optional explicit sync -> learner-progress read ->
knowledge derivation -> material/example retrieval -> generation and validation
-> result. Generation does not implicitly sync or update learner progress.
Document any persistence of generated results separately from generation.

Keep material and learner-progress responsibilities separate even when one file
or database stores both. A sync result such as `WaniKaniSyncData` is a transfer
value, not a third repository. Publish related material and progress consistently;
refreshing one connection must not overwrite another connection or manual input.
Read a coherent input version for each generation without holding a write
transaction across a model call. Domain separation between material and progress
is independent of separating reads from writes.

For one-off use, pass progress and material directly to the library; an existing
value does not need a store merely to be passed to the next function. In-memory
adapters are valid when storage-shaped access is useful. They provide no durability
after process exit and do not imply offline model execution. The CLI keeps local
files by default; Cloud chooses its stores explicitly. A missing real dependency
must not silently fall back to a scripted model or discard writes.

Use a derived `LearnerKnowledge` value on demand rather than persisting a second
authoritative definition of progress. No command bus, event sourcing, separate
read database, or mandatory nullable factory is required. Generation remains a
costly operation with external effects even if it does not persist its result.
If a materialized knowledge view becomes necessary, identify the input versions
and knowledge-policy version on which it depends.

Code and adapters can remain public while runtime data stays private. Public
prompt basics and future private prompt sets use the same contract; retain the
set version for a generation. Public source code does not imply exporting every
Rust type with `pub`. Start with modules in the existing package; group future
integrations by role (for example `adapters/sources/wanikani`) when those modules
are actually needed. Do not create a crate or repository solely for every port.

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
- Store synchronization start and completion times. `WaniKaniSyncData` contains
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
The persisted `snapshot` key is retained for schema-1 compatibility; it does not
promise an instantaneous view of the remote system. The public Rust type was
renamed from `Snapshot` to `WaniKaniSyncData` on 2026-09-30 without changing the
cache layout. `InvalidSnapshot` error variants became `InvalidSyncData`.
`WaniKaniSyncData` contains the synchronization interval, learner, subjects,
assignments, review statistics, and `unavailable_subjects`. Subject lexical content is tagged
as `kanji`, `vocabulary`, or `kana_vocabulary`; only the first two have readings.
The cache contains normalized Yomibu data, not WaniKani response envelopes. See
`crates/yomibu/src/domain.rs` for the concrete field types and `tests/fixtures` for synthetic
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

Loading and summarizing both validate the sync data; library callers who construct
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
`SyncGuard::replace` validates the proposed sync data and checks the current cache's
account identity before creating a private temporary file. It writes schema 1,
flushes and synchronizes the file, persists it atomically, then synchronizes the
parent directory. New directories use mode 0700; new lock/cache files use 0600
(subject to umask). Existing directory permissions are not broadened or rewritten.
`WriteError::DurabilityUncertain` distinguishes post-replacement directory-sync
failure from errors that leave the destination unreplaced.

Successful sync prints the same cached-observation summary as offline status.
Tests cover mock API → normalized sync data → private cache → credential-free
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
waits use a manually advanced clock; socket I/O runs with real time. A separate
socket-free test checks transient waits immediately before and at the deadlines
of one and two seconds. Independent HTTP failure scenarios run concurrently, with
their own servers and caches and the real retry waits. Permanent HTTP errors,
redirects, credential-bearing pagination, and unsafe/repeated URLs fail without
leaking credentials. Mixed transient failure categories share the same retry
budget.

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
macOS and Linux in parallel for pushes to `main` and pull requests. Superseded
runs for the same branch or PR are cancelled. The workflow uses the repository
toolchain file, requires a committed `Cargo.lock`, passes `--locked` to Clippy and
tests, and verifies the lockfile remains unchanged. Tests use local HTTP servers
and synthetic credentials; CI requires no live WaniKani account.

The pinned Rust cache action restores dependency downloads and compiled
dependencies with platform/compiler/dependency-specific keys. Only `main` saves
caches; PRs restore them. A cache miss still runs the complete quality gates.
Documentation changes also run the gates.

Public API documentation records the caller's responsibilities: hold `SyncGuard`
across fetch and replacement, enable Tokio I/O/time for retrieval, trust any
custom API origin that receives a token, and validate publicly constructed data
before direct use. Loading, summarization, and replacement validate automatically.
HTTP errors remain sanitized; `WriteError::DurabilityUncertain` exposes its
underlying I/O cause without changing the already-replaced outcome.

Quality gates passed on native macOS/arm64, containerized Debian Linux/arm64,
and GitHub-hosted Ubuntu/x86_64 and macOS/arm64. See `PLAN.md` for the exact checks,
TDD evidence, CI timings, and remaining verification limits.

## Rust architecture

Use one Cargo workspace with two packages: `crates/yomibu` for the reusable
library and `crates/yomibu-cli` for the executable named `yomibu`. Dependencies
point from CLI to library; the library has no CLI dependency. Root Cargo commands
cover both members, with one lockfile, toolchain and target directory. Shared
dependency versions and test profiles live in the root manifest. No API package
exists until the HTTP application is implemented.

- The binary owns argument parsing, environment access, runtime startup,
  presentation, and exit status.
- The HTTP adapter asynchronously produces normalized `WaniKaniSyncData`.
- Domain validation and summary calculation are synchronous.
- Cache loading, locking, and persistence are synchronous library operations.
  `App.sync` calls them around async retrieval on the caller's thread. A future
  busy server needs asynchronous storage contracts or bounded blocking execution.
- `App` owns or borrows explicit dependencies through generic parameters. Its
  source and storage traits support the demonstrated WaniKani/file/memory paths;
  no dependency-injection framework or dynamic dispatch is required.
- Expose use cases/reports, narrow extension contracts, adapters, domain data,
  and meaningful typed errors. Keep transport DTOs and storage internals private.
  Root `cache` and `wanikani` paths remain re-exports of the moved adapters.
- Concrete types and functions remain the default for deterministic behavior.
- Use ownership and borrowing deliberately: move data when building the sync data
  and borrow it for read-only work. Avoid cloning solely to bypass ownership
  design problems. Store reads return immutable `Arc<WaniKaniSyncData>` versions;
  summaries own their username and aggregates, without copying source collections.
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

The following defaults concern WaniKani synchronization. Story generation uses the explicit
one-attempt model-request limits, without this retry loop.

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
fails promptly; status can continue reading the previous sync data. Use
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

## Code and repository structure

[ARCHITECTURE.md](ARCHITECTURE.md#files-packages-and-repositories) records the
current tree, planned modules, dependency direction, ownership, composition
examples, and public/private repository boundaries. It distinguishes implemented
sync/status, current story generation and supplied-text analysis from future
Cloud capabilities.
Keep one public code repository. The approved library/CLI workspace split makes
existing responsibilities and dependency boundaries visible before the API slice.
Library production dependencies exclude CLI argument parsing and executable error
reporting; retiring the A1 runner also removes them from library development dependencies.
Library tests/examples live with the library, CLI and combined executable tests
with the CLI. Shared immutable fixtures remain under root `tests/fixtures`, and
the real pinned dictionary remains under root `target/a1/current`.
Add `yomibu-api` when real HTTP behavior is implemented; no placeholder crate or
new shared orchestration framework is required. Private prompts/corpora can use
private stores with public adapter implementations.

HTTP adapter tests live beside private adapter code, permitting local-server
configuration without test-only CLI options. File fault/interruption tests live
beside the file adapter. Shared storage cases and application tests exercise the
same use cases with real file and in-memory backends.

## Rust notes for a Ruby developer

- **Enums model distinct shapes.** Kanji, vocabulary, and kana-only vocabulary
  have different valid fields. Variants express that directly instead of using
  objects with many conditionally meaningful attributes.
- **`Option` preserves absence.** No review-statistic record is different from a
  record containing zero counters. Pattern matching keeps that distinction
  explicit.
- **Ownership clarifies data flow.** Move decoded strings and collections into
  the sync data; borrow it for summaries and serialization. A shared mutable
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
