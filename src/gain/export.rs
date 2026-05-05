//! JSON and CSV export for token savings data.
use crate::tracking::{
    CommandStats, DayStats, DetailedCommandRecord, MonthStats, TelemetrySummarySurface, Tracker,
    WeekStats,
};
use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ExportData {
    pub(crate) schema_version: &'static str,
    pub(crate) summary: ExportSummary,
    // telemetry_summary is internal diagnostics; excluded from the public
    // mycelium-gain-v1 JSON contract (schema has additionalProperties: false).
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    pub(crate) telemetry_summary: TelemetrySummarySurface,
    pub(crate) by_command: Vec<ExportCommandStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) by_project: Option<Vec<ExportProjectStats>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) history: Option<Vec<DetailedCommandRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) daily: Option<Vec<DayStats>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) weekly: Option<Vec<WeekStats>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) monthly: Option<Vec<MonthStats>>,
}

#[derive(Serialize)]
pub(crate) struct ExportSummary {
    pub(crate) total_commands: usize,
    pub(crate) total_input: usize,
    pub(crate) total_output: usize,
    pub(crate) total_saved: usize,
    pub(crate) avg_savings_pct: f64,
    pub(crate) total_time_ms: u64,
    pub(crate) avg_time_ms: u64,
}

#[derive(Serialize)]
pub(crate) struct ExportCommandStats {
    pub(crate) avg_savings_pct: f64,
    pub(crate) command: String,
    pub(crate) count: usize,
    pub(crate) exec_time_ms: u64,
    pub(crate) input_tokens: usize,
    pub(crate) tokens_saved: usize,
}

impl From<CommandStats> for ExportCommandStats {
    fn from(value: CommandStats) -> Self {
        Self {
            avg_savings_pct: value.savings_pct,
            command: value.command,
            count: value.count,
            exec_time_ms: value.exec_time_ms,
            input_tokens: value.input_tokens,
            tokens_saved: value.tokens_saved,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ExportProjectStats {
    pub(crate) project_path: String,
    pub(crate) project_name: String,
    pub(crate) commands: i64,
    pub(crate) saved_tokens: i64,
    pub(crate) avg_savings_pct: f64,
    pub(crate) last_used: String,
}

impl From<crate::tracking::ProjectStats> for ExportProjectStats {
    fn from(value: crate::tracking::ProjectStats) -> Self {
        Self {
            project_path: value.project_path,
            project_name: value.project_name,
            commands: value.commands,
            saved_tokens: value.saved_tokens,
            avg_savings_pct: value.avg_savings_pct,
            last_used: value.last_used,
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "CLI export flags map directly to the supported output selectors"
)]
fn build_export_data(
    tracker: &Tracker,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    history: bool,
    limit: usize,
    project_scope: Option<&str>,
) -> Result<ExportData> {
    let summary = tracker
        .get_summary_filtered(project_scope)
        .context("Failed to load token savings summary from database")?;
    let telemetry_summary = tracker
        .get_telemetry_summary_filtered(project_scope)
        .context("Failed to build deterministic telemetry summary surface")?;
    let by_command = tracker
        .get_by_command_limited(project_scope, limit)
        .context("Failed to load by-command token savings from database")?
        .into_iter()
        .map(ExportCommandStats::from)
        .collect();

    Ok(ExportData {
        schema_version: "1.0",
        summary: ExportSummary {
            total_commands: summary.total_commands,
            total_input: summary.total_input,
            total_output: summary.total_output,
            total_saved: summary.total_saved,
            avg_savings_pct: summary.avg_savings_pct,
            total_time_ms: summary.total_time_ms,
            avg_time_ms: summary.avg_time_ms,
        },
        telemetry_summary,
        by_command,
        by_project: None,
        history: if history {
            Some(
                tracker
                    .get_recent_detailed_filtered(limit, project_scope)
                    .context("Failed to load recent command history from database")?,
            )
        } else {
            None
        },
        daily: if all || daily {
            Some(tracker.get_all_days_filtered(project_scope)?)
        } else {
            None
        },
        weekly: if all || weekly {
            Some(tracker.get_by_week_filtered(project_scope)?)
        } else {
            None
        },
        monthly: if all || monthly {
            Some(tracker.get_by_month_filtered(project_scope)?)
        } else {
            None
        },
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "CLI export flags map directly to the supported output selectors"
)]
pub(crate) fn export_json(
    tracker: &Tracker,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    history: bool,
    limit: usize,
    project_scope: Option<&str>,
) -> Result<()> {
    let export = build_export_data(
        tracker,
        daily,
        weekly,
        monthly,
        all,
        history,
        limit,
        project_scope,
    )?;

    let json = serde_json::to_string_pretty(&export)?;
    println!("{}", json);

    Ok(())
}

pub(crate) fn export_json_projects(tracker: &Tracker) -> Result<()> {
    let mut export = build_export_data(tracker, false, false, false, false, false, 50, None)?;

    let stats = tracker
        .get_by_project()
        .context("Failed to load per-project statistics from database")?;

    export.by_project = Some(stats.into_iter().map(Into::into).collect());

    let json = serde_json::to_string_pretty(&export)?;
    println!("{json}");
    Ok(())
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn export_json_string(
    tracker: &Tracker,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    history: bool,
    limit: usize,
    project_scope: Option<&str>,
) -> Result<String> {
    let export = build_export_data(
        tracker,
        daily,
        weekly,
        monthly,
        all,
        history,
        limit,
        project_scope,
    )?;
    Ok(serde_json::to_string_pretty(&export)?)
}

#[cfg(unix)]
pub(crate) fn export_json_projects_string(tracker: &Tracker) -> Result<String> {
    let mut export = build_export_data(tracker, false, false, false, false, false, 50, None)?;
    let stats = tracker
        .get_by_project()
        .context("Failed to load per-project statistics from database")?;
    export.by_project = Some(stats.into_iter().map(Into::into).collect());
    Ok(serde_json::to_string_pretty(&export)?)
}

pub(crate) fn export_csv(
    tracker: &Tracker,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    project_scope: Option<&str>,
) -> Result<()> {
    if all || daily {
        let days = tracker.get_all_days_filtered(project_scope)?;
        println!("# Daily Data");
        println!(
            "date,commands,input_tokens,output_tokens,saved_tokens,savings_pct,total_time_ms,avg_time_ms"
        );
        for day in days {
            println!(
                "{},{},{},{},{},{:.2},{},{}",
                day.date,
                day.commands,
                day.input_tokens,
                day.output_tokens,
                day.saved_tokens,
                day.savings_pct,
                day.total_time_ms,
                day.avg_time_ms
            );
        }
        println!();
    }

    if all || weekly {
        let weeks = tracker.get_by_week_filtered(project_scope)?;
        println!("# Weekly Data");
        println!(
            "date,week_end,commands,input_tokens,output_tokens,saved_tokens,savings_pct,total_time_ms,avg_time_ms"
        );
        for week in weeks {
            println!(
                "{},{},{},{},{},{},{:.2},{},{}",
                week.date,
                week.week_end,
                week.commands,
                week.input_tokens,
                week.output_tokens,
                week.saved_tokens,
                week.savings_pct,
                week.total_time_ms,
                week.avg_time_ms
            );
        }
        println!();
    }

    if all || monthly {
        let months = tracker.get_by_month_filtered(project_scope)?;
        println!("# Monthly Data");
        println!(
            "date,commands,input_tokens,output_tokens,saved_tokens,savings_pct,total_time_ms,avg_time_ms"
        );
        for month in months {
            println!(
                "{},{},{},{},{},{:.2},{},{}",
                month.date,
                month.commands,
                month.input_tokens,
                month.output_tokens,
                month.saved_tokens,
                month.savings_pct,
                month.total_time_ms,
                month.avg_time_ms
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_export_data;
    use crate::tracking::Tracker;
    use tempfile::tempdir;

    #[test]
    fn gain_json_export_includes_deterministic_telemetry_summary_surface() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("gain-export.db");
        let db_path = db_path.to_string_lossy().to_string();
        let tracker = Tracker::new_with_override(Some(&db_path)).expect("tracker");

        tracker
            .record("git diff", "mycelium git diff", 100, 10, 4)
            .expect("record git diff");
        tracker
            .record("git status", "mycelium git status", 100, 10, 5)
            .expect("record git status");

        let export = build_export_data(&tracker, false, false, false, false, false, 10, None)
            .expect("build export data");

        assert_eq!(
            export.telemetry_summary.summary_surface,
            "deterministic-telemetry-summary"
        );
        assert_eq!(export.telemetry_summary.command_breakdown.len(), 2);
        assert_eq!(
            export.telemetry_summary.command_breakdown[0].command,
            "mycelium git diff"
        );
        assert_eq!(
            export.telemetry_summary.command_breakdown[1].command,
            "mycelium git status"
        );
    }

    #[test]
    fn gain_json_project_export_includes_by_project_and_summary() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("gain-project-export.db");
        let db_path = db_path.to_string_lossy().to_string();
        let tracker = Tracker::new_with_override(Some(&db_path)).expect("tracker");

        tracker
            .record("git status", "mycelium git status", 100, 10, 5)
            .expect("record");

        let export = build_export_data(&tracker, false, false, false, false, false, 50, None)
            .expect("build export data");

        // The export includes required schema fields
        assert_eq!(export.schema_version, "1.0");
        assert_eq!(export.summary.total_commands, 1);
        assert!(export.by_project.is_none());

        // Verify that by_project can be populated via get_by_project
        let stats = tracker.get_by_project().expect("get_by_project");
        let by_project: Vec<super::ExportProjectStats> =
            stats.into_iter().map(Into::into).collect();
        // Tracker auto-detects project path from CWD, so this should be non-empty
        assert!(!by_project.is_empty());
    }
}
