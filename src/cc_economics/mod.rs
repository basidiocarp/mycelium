//! Claude Code Economics: Spending vs Savings Analysis
//!
//! Combines ccusage (tokens spent) with mycelium tracking (tokens saved) to provide
//! dual-metric economic impact reporting with blended and active cost-per-token.

mod display;
mod export;
pub mod merge;
pub mod models;

use anyhow::{Context, Result};

use crate::tracking::Tracker;

use display::display_text;
use export::{export_csv, export_json};

/// Display or export Claude Code economics (spending vs savings) in text, JSON, or CSV.
///
/// NOTE: cc_economics uses a bool `--project` flag (scopes to cwd only).
/// The gain command supports `--project <name>` with name-based lookup.
/// If economics needs name-based resolution in the future, upgrade this
/// to `Option<String>` and remove the manual conversion below.
#[allow(clippy::too_many_arguments)]
pub fn run(
    project: bool,
    project_path: Option<&str>,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    format: &str,
    verbose: u8,
) -> Result<()> {
    let tracker = Tracker::new().context("Failed to initialize tracking database")?;
    let project_flag = if project { Some(".") } else { None };
    let project_scope = crate::gain::resolve_project_scope(project_flag, project_path, &tracker)?;

    match format {
        "json" => export_json(
            &tracker,
            daily,
            weekly,
            monthly,
            all,
            project_scope.as_deref(),
        ),
        "csv" => export_csv(
            &tracker,
            daily,
            weekly,
            monthly,
            all,
            project_scope.as_deref(),
        ),
        _ => display_text(
            &tracker,
            daily,
            weekly,
            monthly,
            all,
            verbose,
            project_scope.as_deref(),
        ),
    }
}
