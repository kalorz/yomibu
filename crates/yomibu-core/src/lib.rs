//! Yomibu's functional core: domain values and decisions, and the ports they need.
#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::print_stdout,
    clippy::print_stderr
)]

pub mod domain;
pub mod ports;
