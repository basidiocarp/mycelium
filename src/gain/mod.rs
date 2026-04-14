//! Token savings analytics dashboard with history, graphs, and quota tracking.
mod compare;
mod display;
mod export;
mod helpers;
mod rewrite_diagnostics;

use crate::tracking::Tracker;
use anyhow::{Context, Result};

pub(crate) use helpers::resolve_project_scope;

#[allow(clippy::too_many_arguments)]
pub fn run(
    project: Option<&str>,
    project_path: Option<&str>,
    projects: bool,
    diagnostics: bool,
    explain: bool,
    graph: bool,
    history: bool,
    limit: usize,
    quota: bool,
    tier: &str,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    format: &str,
    failures: bool,
    status: bool,
    compare: Option<&str>,
    _verbose: u8,
) -> Result<()> {
    // --compare mode short-circuits all other display logic
    if let Some(cmd_str) = compare {
        return compare::run_compare(cmd_str);
    }

    let tracker = Tracker::new().context("Failed to initialize tracking database")?;

    if status {
        return display::show_status(&tracker);
    }

    // --projects or --project all: per-project breakdown table
    let is_project_all = projects || project.map(|p| p.eq_ignore_ascii_case("all")).unwrap_or(false);
    if is_project_all {
        return match format {
            "json" => export::export_json_projects(&tracker),
            _ => display::show_projects_table(&tracker),
        };
    }

    let project_scope = helpers::resolve_project_scope(project, project_path, &tracker)?;

    if failures {
        return display::show_failures(&tracker, project_scope.as_deref());
    }

    if diagnostics {
        return rewrite_diagnostics::show_rewrite_diagnostics(
            &tracker,
            project_scope.as_deref(),
            explain,
        );
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
                history,
                limit,
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
            limit,
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
