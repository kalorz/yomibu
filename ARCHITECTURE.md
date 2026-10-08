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
| Sentence analysis and checks | [Sudachi](crates/yomibu-components/src/sudachi_dictionary/), [Japanese checks](crates/yomibu-components/src/japanese_constraint_checks/) | [Sudachi](crates/yomibu-components/tests/sudachi.rs), [evaluation](crates/yomibu-components/tests/evaluation.rs), [passage assessment](crates/yomibu-components/tests/passage_assessment.rs) |
| Story selection, preparation and generation | [Learner selection](crates/yomibu-components/src/learner_vocabulary_selection/), [prompt preparation](crates/yomibu-components/src/story_prompt_preparation/), [OpenAI](crates/yomibu-components/src/openai_story_generation/), [workflow](crates/yomibu/src/application/story.rs) | [Selection](crates/yomibu-components/tests/first_run_selection.rs), [planning](crates/yomibu/tests/story_generation_plan.rs), [workflow](crates/yomibu/tests/story_workflow.rs), [OpenAI](crates/yomibu-components/tests/openai.rs) |
| Embeddings, reuse and selection fallback | [Application policy](crates/yomibu/src/application/embeddings.rs), [embedding selection](crates/yomibu-components/src/embedding_vocabulary_selection/), [HTTP](crates/yomibu-components/src/http_embeddings/), [lexical baseline](crates/yomibu-components/src/lexical_embeddings/), [file cache](crates/yomibu-components/src/file_embedding_cache/) | [Preparation](crates/yomibu-components/tests/retrieval_preparation.rs), [transport](crates/yomibu-components/tests/embeddings.rs), [cache](crates/yomibu-components/tests/embedding_cache_file.rs) |
| Dictionary installation and loading | [Installation](crates/yomibu-components/src/sudachi_dictionary/installation.rs) | [Managed loading](crates/yomibu-components/tests/managed_dictionary.rs); installation fault tests live beside the component |

## Execution and ownership

Production dependencies point `yomibu-cli → yomibu → yomibu-components → yomibu-core`,
with `yomibu → yomibu-core` also explicit. CLI tests use core/components directly
for fixtures and adapter comparisons. Core imports no concrete components;
components import no application code.

[Core domain](crates/yomibu-core/src/domain/) owns shared evidence and invariants.
[Capabilities](crates/yomibu-core/src/capabilities/) define source, storage and
embedding contracts. [Pipeline](crates/yomibu-core/src/pipeline/) holds existing
ranked-selection finalization, selection binding and complete embedding-cache assembly.
There is no configurable selection pipeline.

The CLI captures environment values, drives the runtime and renders results.
The application resolves configuration, constructs components, prepares resources,
and applies refresh, authorization and optional-work policies. Constructors do no I/O.
Pure calculations and file operations are synchronous; network calls use the caller's
executor. Current sync file operations run on that caller's thread.

OpenAI owns exact prepared bytes and model/encoding validation. Prompt preparation
owns content, the request-byte cap and support trimming. Assessment uses the full
inventory. Shared assessment results carry the component's typed execution error;
reports project them without rerunning checks. Checked candidate construction
preserves original text and complete UTF-8 sentence spans.

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
