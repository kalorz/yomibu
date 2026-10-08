//! Shared serializable projections of existing results, without execution or I/O.
//! CLI formatting/escaping and future transport responses remain caller concerns.
pub mod analysis;
pub mod candidate;
pub mod story;

pub mod run;
pub mod summary;

pub use yomibu_components::sudachi_dictionary::installation::Verification;
pub use yomibu_core::domain::embedding::EmbeddingCache;
