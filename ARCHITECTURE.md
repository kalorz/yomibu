# Code map

Find the relevant task below, then read its code and tests. [SPEC](SPEC.md) records
intent and constraints; [PLAN](PLAN.md) lists open work.

## Current module map

| Task | Start here | Tests |
| --- | --- | --- |
| CLI arguments and output | [Commands](crates/yomibu-cli/src/commands/), [output](crates/yomibu-cli/src/output/) | [Executable tests](crates/yomibu-cli/tests/) |
| Configuration and application workflows | [Configuration](crates/yomibu/src/configuration/), [story](crates/yomibu/src/application/local.rs), [explicit operations](crates/yomibu/src/application/local/explicit.rs) | [Configuration](crates/yomibu/tests/configuration.rs), [first run](crates/yomibu/tests/local_application.rs), [explicit operations](crates/yomibu/tests/explicit_operations.rs) |
| Source refresh, storage and offline status | [Source policy](crates/yomibu/src/application/source.rs), [sync](crates/yomibu/src/application/mod.rs), [WaniKani](crates/yomibu-components/src/wanikani_source/), [file store](crates/yomibu-components/src/file_learning_store/), [memory store](crates/yomibu-components/src/in_memory_learning_store/) | [App](crates/yomibu/tests/app.rs), [stores](crates/yomibu-components/tests/stores.rs), [summaries](crates/yomibu/tests/summary.rs); fault tests live beside their components |
| Input loading and allowed material | [Input preparation](crates/yomibu/src/application/inputs.rs), [knowledge](crates/yomibu-core/src/domain/knowledge.rs), [inventory](crates/yomibu-core/src/domain/inventory.rs) | [Knowledge](crates/yomibu-core/tests/knowledge.rs), [inventory](crates/yomibu-core/tests/inventory.rs) |
| Optional assessment, progress and reports | [Assessment policy](crates/yomibu/src/application/assessment.rs), [progress](crates/yomibu/src/application/progress.rs), [reports](crates/yomibu/src/reports/) | [First run](crates/yomibu/tests/local_application.rs), [CLI story](crates/yomibu-cli/src/story_tests.rs), [passages](crates/yomibu/tests/passage_generation.rs) |
| Sentence analysis and checks | [Capabilities](crates/yomibu-core/src/capabilities/story.rs), [evidence composition](crates/yomibu-core/src/pipeline/assessment.rs), [Sudachi](crates/yomibu-components/src/sudachi_dictionary/), [Japanese checks](crates/yomibu-components/src/japanese_constraint_checks/) | [Sudachi](crates/yomibu-components/tests/sudachi.rs), [evaluation](crates/yomibu-components/tests/evaluation.rs), [passage assessment](crates/yomibu-components/tests/passage_assessment.rs) |
| Selection composition | [Capability](crates/yomibu-core/src/capabilities/mod.rs), [core finalization](crates/yomibu-core/src/pipeline/selection.rs), [learner operations](crates/yomibu-components/src/learner_vocabulary_selection/), [default pipelines](crates/yomibu/src/application/selection.rs) | [Boundaries and extension](crates/yomibu-core/tests/selection_composition.rs), [operations](crates/yomibu-components/tests/selection_composition.rs), [defaults](crates/yomibu/tests/first_run_selection.rs), [trimming](crates/yomibu/tests/selection_composition.rs) |
| Story preparation and generation | [Typed plan](crates/yomibu-core/src/pipeline/story.rs), [prompt preparation](crates/yomibu-components/src/story_prompt_preparation/), [OpenAI](crates/yomibu-components/src/openai_story_generation/), [workflow and wiring](crates/yomibu/src/application/story.rs) | [Planning](crates/yomibu/tests/story_generation_plan.rs), [substitution](crates/yomibu/tests/story_capabilities.rs), [workflow](crates/yomibu/tests/story_workflow.rs), [OpenAI](crates/yomibu-components/tests/openai.rs) |
| Embeddings, reuse and selection fallback | [Application policy](crates/yomibu/src/application/embeddings.rs), [embedding selection](crates/yomibu-components/src/embedding_vocabulary_selection/), [HTTP](crates/yomibu-components/src/http_embeddings/), [lexical baseline](crates/yomibu-components/src/lexical_embeddings/), [file cache](crates/yomibu-components/src/file_embedding_cache/) | [Preparation](crates/yomibu-components/tests/retrieval_preparation.rs), [transport](crates/yomibu-components/tests/embeddings.rs), [cache](crates/yomibu-components/tests/embedding_cache_file.rs) |
| Dictionary installation and loading | [Installation](crates/yomibu-components/src/sudachi_dictionary/installation.rs) | [Managed loading](crates/yomibu-components/tests/managed_dictionary.rs); installation fault tests live beside the component |

## Execution and ownership

Production dependencies point `yomibu-cli → yomibu → yomibu-components → yomibu-core`,
with `yomibu → yomibu-core` also explicit. CLI tests use core/components directly
for fixtures and adapter comparisons. Core imports no concrete components;
components import no application code.

[Core domain](crates/yomibu-core/src/domain/) owns shared evidence and invariants.
[Capabilities](crates/yomibu-core/src/capabilities/) define source, storage,
embedding, selection, preparation, generation, analysis and assessment contracts.
[Pipeline](crates/yomibu-core/src/pipeline/)
checks each step's candidates and owns target-first finalization and embedding-cache assembly.
Core validates candidates against the full allowed inventory; external steps may restore
allowed entries. The supplied rankers retain filtered subsets.
The application wires default selection steps and prepares embedding evidence before selection;
optional fallback remains application policy.

The CLI captures environment values, drives the runtime and renders results.
The application resolves configuration, constructs components, prepares resources,
and applies refresh, authorization and optional-work policies. Constructors do no I/O.
Pure calculations and file operations are synchronous; network calls use the caller's
executor. Current sync file operations run on that caller's thread.

OpenAI owns exact prepared bytes, options and model/encoding validation. Prompt
preparation owns content, the request-byte cap and support trimming. `PreparedStory<R>`
binds the finalized selection and full-inventory assessment inputs to that request;
generation requires the same associated request type. Library callers inject
capabilities through `StoryPreparer::prepare` and `generate_story_with`.

Assessment consumes one analysis per original sentence. Core composes typed failures
and passage spans; Japanese checks consume evidence without depending on Sudachi.
The application owns optional resource loading and default diagnostics. Reports
project results without rerunning checks. Checked candidate construction preserves
original text and complete UTF-8 sentence spans.

File components own locking and complete publication; the application rechecks
freshness while holding the writer. Sudachi consumes the checked dictionary handle;
verified bytes must remain unchanged for the analyzer's lifetime.

## Files, packages, and repositories

Root [Cargo.toml](Cargo.toml), [Cargo.lock](Cargo.lock), and
[rust-toolchain.toml](rust-toolchain.toml) own build and dependency facts.
Current engineering fixtures live in [tests/fixtures](tests/fixtures/).
Frozen research inputs live in [history](docs/history/README.md).
Member `tests/` directories cover their owners and cross-component workflows.
Adapter tests use real files or local HTTP servers; memory tests cannot prove durability.
[Shared dictionary lookup](tests/support/dictionary.rs) is compiled only for tests.

## Future composition

Read [product direction](SPEC.md#product) and
[storage boundaries](SPEC.md#data-flow-and-storage-boundaries) only when needed.
Future responsibilities are constraints on real work, not APIs to create in advance.
