//! Declarative TOML-based filter extension layer.
//!
//! Compiled Rust filters always take precedence over declarative filters
//! when both match the same command pattern.

pub mod declarative;

pub use declarative::{
    DeclarativeFilter, FilterMeta, TransformConfig, TruncateConfig, load_declarative_filters,
};
