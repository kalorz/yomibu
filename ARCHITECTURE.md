# Code map

Find the relevant task below, then read its code and tests. [SPEC](SPEC.md) records
intent and constraints; [PLAN](PLAN.md) lists open work.

## Current module map

| Task | Start here | Tests |
| --- | --- | --- |
| CLI arguments and output | [Commands](crates/yomibu-cli/src/commands/), [output](crates/yomibu-cli/src/output/) | [Executable tests](crates/yomibu-cli/tests/) |
| Configuration and application workflows | [Configuration](crates/yomibu/src/configuration/), [story workflow](crates/yomibu/src/application/story_workflow.rs), [local application](crates/yomibu/src/application/local.rs), [explicit operations](crates/yomibu/src/application/local/explicit.rs) | [Configuration](crates/yomibu/tests/configuration.rs), [first run](crates/yomibu/tests/local_application.rs), [explicit operations](crates/yomibu/tests/explicit_operations.rs) |
| Source refresh and storage | [sync](crates/yomibu/src/application/mod.rs), [WaniKani](crates/yomibu-components/src/wanikani_source/), [file store](crates/yomibu-components/src/file_learning_store/) | [App](crates/yomibu/tests/app.rs), [stores](crates/yomibu-components/tests/stores.rs), [summaries](crates/yomibu/tests/summary.rs); fault tests live beside their components |
| Input loading and allowed material | [WaniKani mapping](crates/yomibu-components/src/wanikani_source/inventory.rs), [eligibility](crates/yomibu-core/src/domain/knowledge.rs), [inventory preparation](crates/yomibu-core/src/domain/inventory.rs) | [Knowledge](crates/yomibu-core/tests/knowledge.rs), [inventory](crates/yomibu-core/tests/inventory.rs), [WaniKani integration](crates/yomibu-components/tests/wanikani_inventory.rs) |
| Progress and reports | [progress](crates/yomibu/src/application/progress.rs), [reports](crates/yomibu/src/reports/) | [First run](crates/yomibu/tests/local_application.rs), [CLI story](crates/yomibu-cli/src/story_tests.rs), [passages](crates/yomibu/tests/passage_generation.rs) |
| Selection composition | [Capability](crates/yomibu-core/src/capabilities/mod.rs), [core finalization](crates/yomibu-core/src/pipeline/selection.rs), [learner operations](crates/yomibu-components/src/learner_vocabulary_selection/), [default pipelines](crates/yomibu/src/application/selection.rs) | [Boundaries and extension](crates/yomibu-core/tests/selection_composition.rs), [operations](crates/yomibu-components/tests/selection_composition.rs), [defaults](crates/yomibu/tests/first_run_selection.rs), [trimming](crates/yomibu/tests/selection_composition.rs) |
| Story preparation and generation | [Typed plan](crates/yomibu-core/src/pipeline/story.rs), [prompt preparation](crates/yomibu-components/src/story_prompt_preparation/), [OpenAI](crates/yomibu-components/src/openai_story_generation/), [workflow and wiring](crates/yomibu/src/application/story_workflow.rs) | [Planning](crates/yomibu/tests/story_generation_plan.rs), [substitution](crates/yomibu/tests/story_capabilities.rs), [workflow](crates/yomibu/tests/local_application.rs), [OpenAI](crates/yomibu-components/tests/openai.rs) |

## Execution and ownership

Production dependencies point `yomibu-cli → yomibu → yomibu-components → yomibu-core`,
with `yomibu → yomibu-core` also explicit. CLI tests use components directly
for fixtures and adapter comparisons. Core imports no concrete components;
components import no application code.

[Core domain](crates/yomibu-core/src/domain/) owns shared evidence and invariants.
[Capabilities](crates/yomibu-core/src/capabilities/) define source, storage,
selection, preparation and generation contracts.
[Pipeline](crates/yomibu-core/src/pipeline/)
checks each step's candidates and owns target-first finalization.
Core validates candidates against the full allowed inventory; external steps may restore
allowed entries. The supplied rankers retain filtered subsets.
The application wires default selection steps.

Components declare typed option keys in one [static catalog](crates/yomibu-components/src/lib.rs).
CLI arguments, environment names, file validation and auth enumerate those declarations.
Shared [component options](crates/yomibu-core/src/component/) use typed keys and validators.
The application passes each component its scoped options; credentials retain lazy lookup.
Component declarations also supply help and setup guidance.

The CLI captures environment values, drives the runtime and renders results.
The application resolves scoped settings, immutable invocation overrides and
credentials, constructs components through explicit typed mappings, prepares resources,
and applies authorization. Constructors do no I/O.
Pure calculations and file operations are synchronous; network calls use the caller's
executor. Current sync file operations run on that caller's thread.

OpenAI owns exact prepared bytes, options and model/encoding validation. Prompt
preparation owns content, the request-byte cap and support trimming. `PreparedStory<R>`
binds the finalized selection to that request; generation requires the same associated
request type and checks the caller's count and format. Library callers inject
capabilities through `prepare_story` and `PreparedStory::generate`.
Checked candidate construction preserves original text and complete UTF-8 sentence spans.

File components own locking and complete publication.
The file adapter exposes normalized material, progress and access expiry.
The file adapter owns WaniKani mapping; story generation has no source or writer.

## Files, packages, and repositories

Root [Cargo.toml](Cargo.toml), [Cargo.lock](Cargo.lock), and
[rust-toolchain.toml](rust-toolchain.toml) own build and dependency facts.
Current engineering fixtures live in [tests/fixtures](tests/fixtures/).
Frozen research inputs live in [history](docs/history/README.md).
Member `tests/` directories cover their owners and cross-component workflows.
Adapter tests use real files or local HTTP servers.

## Future composition

Read [product direction](SPEC.md#product) and
[storage boundaries](SPEC.md#data-flow-and-storage-boundaries) only when needed.
Future responsibilities are constraints on real work, not APIs to create in advance.
