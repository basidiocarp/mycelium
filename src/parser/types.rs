//! Canonical types for tool outputs (test results, dependencies, diagnostics).
use serde::{Deserialize, Serialize};

/// Test execution result (vitest, playwright, jest, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: Option<u64>,
    pub failures: Vec<TestFailure>,
    #[serde(default)]
    pub passed_names: Vec<String>,
}

/// A single test failure with location and error details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFailure {
    pub test_name: String,
    pub file_path: String,
    pub error_message: String,
    pub stack_trace: Option<String>,
}

/// Dependency state (pnpm, npm, cargo, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyState {
    pub total_packages: usize,
    pub outdated_count: usize,
    pub dependencies: Vec<Dependency>,
}

/// A single package dependency with version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub wanted_version: Option<String>,
    pub dev_dependency: bool,
}

/// Severity level for a diagnostic message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A single compiler/linter diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub context: Vec<String>,
}

/// Aggregated diagnostics report (TypeScript, ESLint, Pylint, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub tool: String,
    pub total_errors: usize,
    pub total_warnings: usize,
    pub files_affected: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub by_code: Vec<(String, usize)>, // sorted desc by count
    #[serde(default)]
    pub global_messages: Vec<String>,
}

// ── GitHub CLI types ────────────────────────────────────────────────────────

/// A single GitHub issue in a list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhIssueItem {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub author: String,
}

/// A list of GitHub issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhIssueList {
    pub issues: Vec<GhIssueItem>,
}

/// A single GitHub issue with full details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhIssueDetail {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub url: String,
    pub body: String,
}

/// A single workflow run in a list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhRunItem {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// A list of GitHub workflow runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhRunList {
    pub runs: Vec<GhRunItem>,
}

/// A workflow run summary from `gh run view`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhRunViewSummary {
    #[serde(default)]
    pub run_id: Option<String>,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    #[serde(default)]
    pub failed_jobs: Vec<String>,
}

/// GitHub repository details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhRepoDetail {
    pub owner: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub stars: i64,
    pub forks: i64,
    pub private: bool,
}
