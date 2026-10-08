# Code map

Find the relevant task below, then read its code and tests. [SPEC](SPEC.md) records
intent and constraints; [PLAN](PLAN.md) lists open work.

## Current module map

| Task | Start here | Tests |
| --- | --- | --- |
| CLI arguments and output | [CLI source](crates/yomibu-cli/src/) | [Executable tests](crates/yomibu-cli/tests/) |
| Configuration and application workflows | [Configuration](crates/yomibu/src/app/config.rs), [story](crates/yomibu/src/app/local.rs), [explicit operations](crates/yomibu/src/app/local/explicit.rs) | [Configuration](crates/yomibu/tests/configuration.rs), [first run](crates/yomibu/tests/local_application.rs), [explicit operations](crates/yomibu/tests/explicit_operations.rs) |
| Source refresh, cache reuse and offline status | [Source policy](crates/yomibu/src/app/source.rs), [app.rs](crates/yomibu/src/app.rs), [WaniKani adapter](crates/yomibu/src/adapters/sources/wanikani/), [file store](crates/yomibu/src/adapters/stores/file/) | [First run](crates/yomibu/tests/local_application.rs), [app](crates/yomibu/tests/app.rs), [summaries](crates/yomibu/tests/summary.rs); adapter fault tests live beside their source |
| Input loading and allowed learner material | [Input and inventory preparation](crates/yomibu/src/app/inputs.rs), [knowledge.rs](crates/yomibu/src/knowledge.rs) → [inventory.rs](crates/yomibu/src/inventory.rs) | [Explicit operations](crates/yomibu/tests/explicit_operations.rs), [knowledge](crates/yomibu/tests/knowledge.rs), [inventory](crates/yomibu/tests/inventory.rs) |
| Optional assessment, progress and reports | [Assessment policy](crates/yomibu/src/app/assessment.rs), [reporting](crates/yomibu/src/app/reporting.rs) | [First run](crates/yomibu/tests/local_application.rs), [CLI story](crates/yomibu-cli/src/story_tests.rs), [passages](crates/yomibu/tests/passage_generation.rs) |
| Sentence analysis and checks | [Sudachi adapter](crates/yomibu/src/adapters/sudachi.rs), [evaluation](crates/yomibu/src/evaluation/) | [Sudachi](crates/yomibu/tests/sudachi.rs), [evaluation](crates/yomibu/tests/evaluation.rs) |
| Story planning and generation | [story/mod.rs](crates/yomibu/src/story/mod.rs) | [Shared workflow](crates/yomibu/tests/story_workflow.rs), [assessment](crates/yomibu/tests/story_generation.rs) |
| Embeddings, cache reuse and selection fallback | [Application policy](crates/yomibu/src/app/embeddings.rs), [retrieval.rs](crates/yomibu/src/retrieval.rs), [embedding adapters](crates/yomibu/src/adapters/embeddings.rs) | [First run](crates/yomibu/tests/local_application.rs), [preparation](crates/yomibu/tests/retrieval_preparation.rs), [transport](crates/yomibu/tests/embeddings.rs) |
| Dictionary installation and loading | [Dictionary adapter](crates/yomibu/src/adapters/dictionary.rs), [Sudachi adapter](crates/yomibu/src/adapters/sudachi.rs) | [Managed loading](crates/yomibu/tests/managed_dictionary.rs); installation fault tests live beside the adapter |

`analysis` holds morphological evidence; `evaluation` checks constraints;
`story/assessment` adds target observations and departures from the prompt selection.

## Execution and ownership

The CLI parses arguments, supplies environment values, drives the runtime, and
renders results. The application resolves configuration, prepares resources,
applies cache/capability policies, and emits progress. Constructors do no I/O.
Pure calculations and file operations are synchronous; network calls use the caller's
executor. Current sync file operations run on that caller's thread.

[ports.rs](crates/yomibu/src/ports.rs) defines sync, storage and embedding contracts.
The `story` workflow coordinates concrete OpenAI and Sudachi adapters.
OpenAI owns prepared requests; `candidate` owns generated texts, provider metadata
and common assessments. [Story preparation](crates/yomibu/src/story/preparation.rs)
owns the request-byte cap and support trimming. Reports do not rerun assessment.

## Files, packages, and repositories

The workspace has a library and a CLI, with dependencies from CLI to library.
Root [Cargo.toml](Cargo.toml), [Cargo.lock](Cargo.lock), and
[rust-toolchain.toml](rust-toolchain.toml) own build and dependency facts.
Current engineering fixtures live in [tests/fixtures](tests/fixtures/).
Frozen research inputs live in [history](docs/history/README.md).
Member `tests/` directories cover library and executable behavior. Adapter fault
tests use real files or local HTTP servers; memory tests cannot prove durability.
Both crates include [shared dictionary lookup](tests/support/dictionary.rs) with
`#[path]`; it is compiled only for tests.

## Future composition

Read [product direction](SPEC.md#product) and
[storage boundaries](SPEC.md#data-flow-and-storage-boundaries) only when the task
needs them. Future responsibilities are constraints on real work, not APIs to
create in advance. Keep the current functions and modules until a concrete need
justifies another abstraction.
