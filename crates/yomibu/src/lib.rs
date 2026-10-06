//! WaniKani synchronization, offline preparation, preview, and bounded A1 analysis.
//!
//! [`preview::preview`] synchronously selects structured word entries and checks
//! membership/count. It borrows explicit inputs and needs no account, store, or
//! runtime. Grammar and linguistic correctness remain explicitly unassessed.
//!
//! [`preparation::prepare_context`] derives revisable knowledge and retrieves
//! cached lexical evidence for explicit word/reading/sense targets. It borrows
//! source observations, validated manual grammar declarations, and targets;
//! no store or runtime is required. [`knowledge::LearnerKnowledgePolicy`] selects
//! the concrete source-evidence rule. [`adapters::grammar_file`] explicitly loads
//! versioned manual assertions without writing or interpreting grammar.
//! Retrieval does not establish reading/sense association or example suitability.
//!
//! [`adapters::sudachi::SudachiAnalyzer`] explicitly loads a checksum-pinned
//! dictionary for offline C/A morphology with original UTF-8 spans.
//! [`evaluation::evaluate`] applies bounded synthetic word/grammar permissions,
//! preserving [`grammar::GrammarDeclarations`] without interpreting their text.
//! Completed judgments, uncertainty, execution errors, and checks not run remain
//! distinct. A1 does not establish accepted exercises, naturalness, idioms, or
//! contextual reading/sense correctness. See the `a1` example for composition.
//!
//! Current story generation starts with [`inventory::LearnerInventory`] from
//! manual data or a WaniKani projection, then [`story::StoryRequest`] supplies
//! a brief and vocabulary/grammar targets. [`story::select_vocabulary`] uses an
//! explicit validated embedding cache. [`story::build_ai_model_request`] freezes
//! the outbound bytes before execution resources are initialized.
//! [`adapters::openai::Client::generate_story_candidates`] sends those bytes;
//! [`story::assess_candidates`] checks every text against the full inventory.
//! [`story::StoryGenerationOptions`] sets candidate count separately from story intent.
//! See `docs/STORY_GENERATION.md` for the complete sequence. Historical G1/G2 evidence
//! is recorded in `docs/GENERATION_HISTORY.md`.
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
//! enabled. File operations, validation, summaries, and preparation are synchronous.
//! Read [`App::sync`] for cancellation and persistence outcomes; backend errors remain typed.
//! [`app::SyncReport`] distinguishes volatile retention from durable persistence.
//!
//! Lower-level access remains available through [`wanikani::Client::fetch`],
//! [`cache::SyncGuard`], and [`cache::load`]. Direct callers must keep their writer
//! guard alive across retrieval and replacement. Root `cache` and `wanikani`
//! module paths re-export the adapters for existing callers.
//!
//! [`cache::load`] and [`domain::WaniKaniSyncData::summarize`] support offline inspection.
//! Public domain fields allow callers to construct data; loading, replacement,
//! summarization, knowledge derivation, and preparation each validate it. This
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
pub mod preparation;
pub mod preview;
pub mod retrieval;
pub mod story;
pub mod summary;
pub use adapters::sources::wanikani;
