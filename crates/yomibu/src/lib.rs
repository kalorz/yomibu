//! WaniKani synchronization, story generation and bounded sentence analysis.
//!
//! - [`App`] synchronizes source data and reads offline status.
//! - [`story::plan_generation`] prepares; [`story::generate_story`] executes.
//! - [`adapters::sudachi::SudachiAnalyzer`] supplies morphology; [`evaluation::evaluate`] checks it.
//! - [`retrieval::prepare_cache`] prepares vectors with a caller-selected encoder.
//!
//! Callers choose files, credentials and runtimes. The library reads no environment
//! variables and prints no output. Bounded checks do not establish exercise acceptance.

pub mod adapters;
pub mod analysis;
pub mod app;
pub use app::App;
pub mod candidate;
pub mod domain;
pub mod evaluation;
pub mod grammar;
pub mod inventory;
pub mod knowledge;
pub mod ports;
pub mod reports;
pub mod retrieval;
pub mod story;
pub mod summary;
