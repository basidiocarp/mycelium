//! Declarative TOML-based filter extension layer.
//!
//! Compiled Rust filters always take precedence over declarative filters
//! when both match the same command pattern.

pub mod declarative;

// Re-exported as part of the library surface; the binary may not use all of these directly.
#[allow(unused_imports)]
pub use declarative::{
    DeclarativeFilter, find_matching_filter, load_all_declarative_filters, load_declarative_filters,
};
