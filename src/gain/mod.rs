//! Token savings analytics dashboard with history, graphs, and quota tracking.
mod compare;
mod display;
mod export;
mod helpers;

use crate::tracking::Tracker;
use anyhow::{Context, Result};

#[allow(clippy::too_many_arguments)]
pub fn run(
    project: bool,
    graph: bool,
    history: bool,
    quota: bool,
    tier: &str,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    format: &str,
    failures: bool,
    compare: Option<&str>,
    _verbose: u8,
) -> Result<()> {
    // --compare mode short-circuits all other display logic
    if let Some(cmd_str) = compare {
        return compare::run_compare(cmd_str);
    }

    let tracker = Tracker::new().context("Failed to initialize tracking database")?;
    let project_scope = helpers::resolve_project_scope(project)?;

    if failures {
        return display::show_failures(&tracker);
    }

    // Handle export formats
    match format {
        "json" => {
            return export::export_json(
                &tracker,
                daily,
                weekly,
                monthly,
                all,
                project_scope.as_deref(),
            );
        }
        "csv" => {
            return export::export_csv(
                &tracker,
                daily,
                weekly,
                monthly,
                all,
                project_scope.as_deref(),
            );
        }
        _ => {} // Continue with text format
    }

    // Default view (summary)
    if !daily && !weekly && !monthly && !all {
        return display::show_summary(
            &tracker,
            project_scope.as_deref(),
            graph,
            history,
            quota,
            tier,
        );
    }

    // Time breakdown views
    if all || daily {
        display::print_daily_full(&tracker, project_scope.as_deref())?;
    }

    if all || weekly {
        display::print_weekly(&tracker, project_scope.as_deref())?;
    }

    if all || monthly {
        display::print_monthly(&tracker, project_scope.as_deref())?;
    }

    Ok(())
}
