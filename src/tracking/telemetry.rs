//! Deterministic telemetry summary surfaces built from local tracking aggregates.
//!
//! These summaries are local-first and machine-readable. They intentionally reuse
//! existing tracking and gain aggregates instead of introducing a separate remote
//! telemetry path.

use anyhow::Result;
use serde::Serialize;

use super::{DayStats, Tracker};

const TELEMETRY_SUMMARY_SCHEMA_VERSION: &str = "1.0";
const TELEMETRY_SUMMARY_SURFACE: &str = "deterministic-telemetry-summary";

/// Stable machine-readable telemetry summary surface for operator tooling.
///
/// `cortina` captures normalized edge events, `mycelium` summarizes them into
/// this deterministic local surface, and `cap` or other operator tooling can
/// consume it without recomputing aggregates.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetrySummarySurface {
    pub schema_version: &'static str,
    pub summary_surface: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_scope: Option<String>,
    pub totals: TelemetryTotals,
    pub command_breakdown: Vec<TelemetryCommandSummary>,
    pub daily_rollup: Vec<TelemetryDaySummary>,
    pub passthrough_summary: TelemetryPassthroughSummary,
    pub parse_failure_summary: TelemetryParseFailureSummary,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetryTotals {
    pub total_commands: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_saved_tokens: usize,
    pub avg_savings_pct: f64,
    pub total_exec_time_ms: u64,
    pub avg_exec_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetryCommandSummary {
    pub command: String,
    pub executions: usize,
    pub input_tokens: usize,
    pub saved_tokens: usize,
    pub avg_savings_pct: f64,
    pub avg_exec_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetryDaySummary {
    pub date: String,
    pub commands: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub saved_tokens: usize,
    pub savings_pct: f64,
    pub total_exec_time_ms: u64,
    pub avg_exec_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetryPassthroughSummary {
    pub total_commands: usize,
    pub total_exec_time_ms: u64,
    pub top_commands: Vec<TelemetryPassthroughCommandSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetryPassthroughCommandSummary {
    pub command: String,
    pub executions: usize,
    pub total_exec_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetryParseFailureSummary {
    pub total_failures: usize,
    pub recovery_rate: f64,
    pub top_commands: Vec<TelemetryParseFailureCommandSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TelemetryParseFailureCommandSummary {
    pub command: String,
    pub failures: usize,
}

impl From<DayStats> for TelemetryDaySummary {
    fn from(value: DayStats) -> Self {
        Self {
            date: value.date,
            commands: value.commands,
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            saved_tokens: value.saved_tokens,
            savings_pct: value.savings_pct,
            total_exec_time_ms: value.total_time_ms,
            avg_exec_time_ms: value.avg_time_ms,
        }
    }
}

impl Tracker {
    /// Build the named deterministic telemetry summary surface from local tracking aggregates.
    pub fn get_telemetry_summary_filtered(
        &self,
        project_path: Option<&str>,
    ) -> Result<TelemetrySummarySurface> {
        let gain_summary = self.get_summary_filtered(project_path)?;
        let passthrough = self.get_passthrough_summary_filtered(project_path)?;
        let parse_failures = self.get_parse_failure_summary_filtered(project_path)?;
        let daily_rollup = self
            .get_all_days_filtered(project_path)?
            .into_iter()
            .map(TelemetryDaySummary::from)
            .collect();

        Ok(TelemetrySummarySurface {
            schema_version: TELEMETRY_SUMMARY_SCHEMA_VERSION,
            summary_surface: TELEMETRY_SUMMARY_SURFACE,
            project_scope: project_path.map(ToOwned::to_owned),
            totals: TelemetryTotals {
                total_commands: gain_summary.total_commands,
                total_input_tokens: gain_summary.total_input,
                total_output_tokens: gain_summary.total_output,
                total_saved_tokens: gain_summary.total_saved,
                avg_savings_pct: gain_summary.avg_savings_pct,
                total_exec_time_ms: gain_summary.total_time_ms,
                avg_exec_time_ms: gain_summary.avg_time_ms,
            },
            command_breakdown: gain_summary
                .by_command
                .into_iter()
                .map(|command| TelemetryCommandSummary {
                    command: command.command,
                    executions: command.count,
                    input_tokens: command.input_tokens,
                    saved_tokens: command.tokens_saved,
                    avg_savings_pct: command.savings_pct,
                    avg_exec_time_ms: command.exec_time_ms,
                })
                .collect(),
            daily_rollup,
            passthrough_summary: TelemetryPassthroughSummary {
                total_commands: passthrough.total_commands,
                total_exec_time_ms: passthrough.total_exec_time_ms,
                top_commands: passthrough
                    .top_commands
                    .into_iter()
                    .map(|command| TelemetryPassthroughCommandSummary {
                        command: command.command,
                        executions: command.count,
                        total_exec_time_ms: command.total_exec_time_ms,
                    })
                    .collect(),
            },
            parse_failure_summary: TelemetryParseFailureSummary {
                total_failures: parse_failures.total,
                recovery_rate: parse_failures.recovery_rate,
                top_commands: parse_failures
                    .top_commands
                    .into_iter()
                    .map(|(command, failures)| TelemetryParseFailureCommandSummary {
                        command,
                        failures,
                    })
                    .collect(),
            },
        })
    }
}
