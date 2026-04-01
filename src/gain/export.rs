//! JSON and CSV export for token savings data.
use crate::tracking::{
    CommandStats, DayStats, DetailedCommandRecord, MonthStats, Tracker, WeekStats,
};
use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ExportData {
    pub(crate) schema_version: &'static str,
    pub(crate) summary: ExportSummary,
    pub(crate) by_command: Vec<ExportCommandStats>,
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
    let summary = tracker
        .get_summary_filtered(project_scope)
        .context("Failed to load token savings summary from database")?;
    let by_command = tracker
        .get_by_command_limited(project_scope, limit)
        .context("Failed to load by-command token savings from database")?
        .into_iter()
        .map(ExportCommandStats::from)
        .collect();

    let export = ExportData {
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
        by_command,
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
    };

    let json = serde_json::to_string_pretty(&export)?;
    println!("{}", json);

    Ok(())
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
            "week_start,week_end,commands,input_tokens,output_tokens,saved_tokens,savings_pct,total_time_ms,avg_time_ms"
        );
        for week in weeks {
            println!(
                "{},{},{},{},{},{},{:.2},{},{}",
                week.week_start,
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
            "month,commands,input_tokens,output_tokens,saved_tokens,savings_pct,total_time_ms,avg_time_ms"
        );
        for month in months {
            println!(
                "{},{},{},{},{},{:.2},{},{}",
                month.month,
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
