//! WaniKani synchronization, shared story generation and bounded supplied-text analysis.
//!
//! Start with the adjacent [`story::plan_generation`] and [`story::generate_story`]
//! functions in `story/mod.rs`. Their concrete stage calls lead to sibling
//! `request.rs`, `selection.rs`, `model_request.rs` and `assessment.rs` files.
//! Private child modules re-export their public types/functions under `story`;
//! existing imports stay unchanged. Smaller cohesive modules remain flat.
//!
//! Story generation starts with [`inventory::LearnerInventory`] from
//! manual data or a WaniKani projection, then [`story::StoryRequest`] supplies
//! a brief and vocabulary/grammar targets. [`story::plan_generation`] validates
//! inputs, selects vocabulary once and freezes the exact request and full-inventory
//! assessment inputs before execution resources are initialized.
//! [`story::generate_story`] makes one provider attempt and assesses every text,
//! returning owned originals and partial results. Callers provide the client,
//! analyzer and async executor; the library starts no runtime or environment lookup.
//! [`story::StoryGenerationOptions`] sets candidate count separately from story intent.
//! See `docs/STORY_GENERATION.md` for usage and `docs/GENERATION_HISTORY.md` for history.
//!
//! [`knowledge::LearnerKnowledgePolicy`] derives eligibility from preserved
//! WaniKani evidence without grammar input, I/O or persistence. Historical manual
//! preview and source-inspection commands are retired; see `docs/COMMAND_HISTORY.md`.
//!
//! [`adapters::sudachi::SudachiAnalyzer`] explicitly loads a checksum-pinned
//! dictionary for offline C/A morphology with original UTF-8 spans.
//! `evaluation/mod.rs` holds result types and check outcomes; `structure.rs` owns
//! supported constructions and safeguards, and `inventory.rs` owns full-inventory
//! checks and single-use evidence. Evaluation has no dependency on story.
//! [`evaluation::evaluate`] applies bounded synthetic word/grammar permissions,
//! preserving [`grammar::GrammarDeclarations`] without interpreting their text.
//! Completed judgments, uncertainty, execution errors, and checks not run remain
//! distinct. A1 does not establish accepted exercises, naturalness, idioms, or
//! contextual reading/sense correctness. Historical A1 evidence is in
//! `docs/A1_IMPLEMENTATION.md`; its research runner is retired.
//!
//! [`App`] coordinates explicit synchronization and offline status for a single
//! account-scoped store. Supply a file or in-memory store and, for synchronization,
//! a source. There is no implicit sync or current-user selection.
//!
//! ```no_run
//! use yomibu::{App, adapters::{sources::wanikani::Client, stores::FileLearningStore}};
//!
//! # async fn example(token: &str) -> Result<(), Box<dyn std::error::Error>> {
//! let mut app = App::new(FileLearningStore::new("/path/to/yomibu-data"))
//!     .with_source(Client::new(token)?);
//! let report = app.sync().await?;
//! let summary = app.status()?;
//! assert_eq!(report.summary, summary);
//! # Ok(())
//! # }
//! ```
//!
//! Network synchronization needs a caller-owned Tokio runtime with I/O and time
//! enabled. File operations, validation, summaries, and planning are synchronous.
//! Read [`App::sync`] for cancellation and persistence outcomes; backend errors remain typed.
//! [`app::SyncReport`] distinguishes volatile retention from durable persistence.
//!
//! Lower-level access remains available through [`wanikani::Client::fetch`],
//! [`cache::SyncGuard`], and [`cache::load`]. Direct callers must keep their writer
//! guard alive across retrieval and replacement. Root `cache` and `wanikani`
//! module paths re-export the adapters for existing callers.
//!
//! [`retrieval::prepare_cache`] reuses compatible vectors and prepares missing
//! inputs through a caller-selected encoder. [`adapters::embedding_cache_file`]
//! and [`adapters::input_file`] accept explicit paths without environment lookup.
//! [`reports`] exposes serializable story/candidate/analysis projections;
//! terminal formatting and escaping remain the executable's responsibility.
//!
//! [`cache::load`] and [`domain::WaniKaniSyncData::summarize`] support offline inspection.
//! Public domain fields allow callers to construct data; loading, replacement,
//! summarization, knowledge derivation, and story planning each validate it. This
//! library does not read credentials or paths from the environment, start a
//! runtime, or print output.

pub mod adapters;
pub mod analysis;
pub mod app;
pub use adapters::stores::file::cache;
pub use app::App;
pub mod domain;
pub mod evaluation;
pub mod generation;
pub mod grammar;
pub mod inventory;
pub mod knowledge;
pub mod ports;
pub mod reports;
pub mod retrieval;
pub mod story;
pub mod summary;
pub use adapters::sources::wanikani;
