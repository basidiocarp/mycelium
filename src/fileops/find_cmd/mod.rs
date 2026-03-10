//! Gitignore-aware file finder with glob matching and grouped output.
pub mod parser;
pub mod search;

pub use search::run;

use anyhow::Result;
use parser::parse_find_args;

/// Parse raw find arguments (native or Mycelium syntax) and run the search.
pub fn run_from_args(args: &[String], verbose: u8) -> Result<()> {
    let parsed = parse_find_args(args)?;
    run(
        &parsed.pattern,
        &parsed.path,
        parsed.max_results,
        parsed.max_depth,
        &parsed.file_type,
        parsed.case_insensitive,
        verbose,
    )
}
