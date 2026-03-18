//! Gitignore-aware file finder with glob matching and grouped output.
pub mod parser;
pub mod search;

pub use search::run;

use std::process::Command;

use anyhow::{Context, Result};
use parser::parse_find_args;

/// Parse raw find arguments (native or Mycelium syntax) and run the search.
/// Falls through to the real `find` binary for unsupported flag combinations.
pub fn run_from_args(args: &[String], verbose: u8) -> Result<()> {
    match parse_find_args(args) {
        Ok(parsed) => run(
            &parsed.pattern,
            &parsed.path,
            parsed.max_results,
            parsed.max_depth,
            &parsed.file_type,
            parsed.case_insensitive,
            verbose,
        ),
        Err(_) => {
            if verbose > 0 {
                eprintln!("find passthrough: {:?}", args);
            }
            let status = Command::new("find")
                .args(args)
                .status()
                .context("Failed to run find")?;
            std::process::exit(status.code().unwrap_or(1));
        }
    }
}
