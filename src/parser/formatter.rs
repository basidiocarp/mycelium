//! Token-efficient formatting trait and modes for canonical output types.
use super::types::{
    DependencyState, Diagnostic, DiagnosticReport, GhIssueDetail, GhIssueList, GhRepoDetail,
    GhRunList, TestResult,
};

/// Output formatting modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    /// Ultra-compact: Summary only (default)
    Compact,
    /// Verbose: Include details
    Verbose,
    /// Ultra-compressed: Symbols and abbreviations
    Ultra,
}

impl FormatMode {
    /// Map a CLI verbosity level (0, 1, 2+) to a format mode.
    pub fn from_verbosity(verbosity: u8) -> Self {
        match verbosity {
            0 => FormatMode::Compact,
            1 => FormatMode::Verbose,
            _ => FormatMode::Ultra,
        }
    }
}

/// Trait for formatting canonical types into token-efficient strings
pub trait TokenFormatter {
    /// Format as compact summary (default)
    fn format_compact(&self) -> String;

    /// Format with details (verbose mode)
    fn format_verbose(&self) -> String;

    /// Format with symbols (ultra-compressed mode)
    fn format_ultra(&self) -> String;

    /// Format according to mode
    fn format(&self, mode: FormatMode) -> String {
        match mode {
            FormatMode::Compact => self.format_compact(),
            FormatMode::Verbose => self.format_verbose(),
            FormatMode::Ultra => self.format_ultra(),
        }
    }
}

impl TokenFormatter for TestResult {
    fn format_compact(&self) -> String {
        let mut lines = vec![format!("PASS ({}) FAIL ({})", self.passed, self.failed)];

        if !self.failures.is_empty() {
            lines.push(String::new());
            for (idx, failure) in self.failures.iter().enumerate().take(5) {
                lines.push(format!("{}. {}", idx + 1, failure.test_name));
                let error_preview: String = failure
                    .error_message
                    .lines()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(format!("   {}", error_preview));
            }

            if self.failures.len() > 5 {
                lines.push(format!("\n... +{} more failures", self.failures.len() - 5));
            }
        }

        if let Some(duration) = self.duration_ms {
            lines.push(format!("\nTime: {}ms", duration));
        }

        lines.join("\n")
    }

    fn format_verbose(&self) -> String {
        let mut lines = vec![format!(
            "Tests: {} passed, {} failed, {} skipped (total: {})",
            self.passed, self.failed, self.skipped, self.total
        )];

        if !self.failures.is_empty() {
            lines.push("\nFailures:".to_string());
            for (idx, failure) in self.failures.iter().enumerate() {
                lines.push(format!(
                    "\n{}. {} ({})",
                    idx + 1,
                    failure.test_name,
                    failure.file_path
                ));
                lines.push(format!("   {}", failure.error_message));
                if let Some(stack) = &failure.stack_trace {
                    let stack_preview: String =
                        stack.lines().take(3).collect::<Vec<_>>().join("\n   ");
                    lines.push(format!("   {}", stack_preview));
                }
            }
        }

        if let Some(duration) = self.duration_ms {
            lines.push(format!("\nDuration: {}ms", duration));
        }

        lines.join("\n")
    }

    fn format_ultra(&self) -> String {
        format!(
            "✓{} ✗{} ⊘{} ({}ms)",
            self.passed,
            self.failed,
            self.skipped,
            self.duration_ms.unwrap_or(0)
        )
    }
}

impl TokenFormatter for DependencyState {
    fn format_compact(&self) -> String {
        if self.outdated_count == 0 {
            return "All packages up-to-date ✓".to_string();
        }

        let mut lines = vec![format!(
            "{} outdated packages (of {})",
            self.outdated_count, self.total_packages
        )];

        for dep in self.dependencies.iter().take(10) {
            if let Some(latest) = &dep.latest_version
                && &dep.current_version != latest
            {
                lines.push(format!(
                    "{}: {} → {}",
                    dep.name, dep.current_version, latest
                ));
            }
        }

        if self.outdated_count > 10 {
            lines.push(format!("\n... +{} more", self.outdated_count - 10));
        }

        lines.join("\n")
    }

    fn format_verbose(&self) -> String {
        let mut lines = vec![format!(
            "Total packages: {} ({} outdated)",
            self.total_packages, self.outdated_count
        )];

        if self.outdated_count > 0 {
            lines.push("\nOutdated packages:".to_string());
            for dep in &self.dependencies {
                if let Some(latest) = &dep.latest_version
                    && &dep.current_version != latest
                {
                    let dev_marker = if dep.dev_dependency { " (dev)" } else { "" };
                    lines.push(format!(
                        "  {}: {} → {}{}",
                        dep.name, dep.current_version, latest, dev_marker
                    ));
                    if let Some(wanted) = &dep.wanted_version
                        && wanted != latest
                    {
                        lines.push(format!("    (wanted: {})", wanted));
                    }
                }
            }
        }

        lines.join("\n")
    }

    fn format_ultra(&self) -> String {
        format!(
            "{} packages (+{} outdated)",
            self.total_packages, self.outdated_count
        )
    }
}

impl TokenFormatter for DiagnosticReport {
    fn format_compact(&self) -> String {
        if self.total_errors == 0 && self.total_warnings == 0 {
            return format!("✓ {}: No errors found", self.tool);
        }

        let mut lines = vec![format!(
            "{}: {} errors in {} files",
            self.tool, self.total_errors, self.files_affected
        )];
        lines.push("═══════════════════════════════════════".to_string());

        // Top error codes summary
        if self.by_code.len() > 1 {
            let codes_str: Vec<String> = self
                .by_code
                .iter()
                .take(5)
                .map(|(code, count)| format!("{} ({}x)", code, count))
                .collect();
            lines.push(format!("Top codes: {}", codes_str.join(", ")));
            lines.push(String::new());
        }

        // Group by file
        let mut by_file: std::collections::HashMap<&str, Vec<&Diagnostic>> =
            std::collections::HashMap::new();
        for d in &self.diagnostics {
            by_file.entry(d.file.as_str()).or_default().push(d);
        }
        let mut files_sorted: Vec<_> = by_file.iter().collect();
        files_sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (file, diags) in &files_sorted {
            lines.push(format!("{} ({} errors)", file, diags.len()));
            for d in *diags {
                let msg: String = d.message.chars().take(120).collect();
                lines.push(format!("  L{}: {} {}", d.line, d.code, msg));
                for ctx in &d.context {
                    let ctx_short: String = ctx.chars().take(120).collect();
                    lines.push(format!("    {}", ctx_short));
                }
            }
            lines.push(String::new());
        }

        lines.join("\n").trim().to_string()
    }

    fn format_verbose(&self) -> String {
        if self.total_errors == 0 && self.total_warnings == 0 {
            return format!("✓ {}: No errors found", self.tool);
        }

        let mut lines = vec![format!(
            "{}: {} errors, {} warnings in {} files",
            self.tool, self.total_errors, self.total_warnings, self.files_affected
        )];
        lines.push("═══════════════════════════════════════".to_string());

        let mut by_file: std::collections::HashMap<&str, Vec<&Diagnostic>> =
            std::collections::HashMap::new();
        for d in &self.diagnostics {
            by_file.entry(d.file.as_str()).or_default().push(d);
        }
        let mut files_sorted: Vec<_> = by_file.iter().collect();
        files_sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (file, diags) in &files_sorted {
            lines.push(format!("{} ({} issues)", file, diags.len()));
            for d in *diags {
                let msg: String = d.message.chars().take(120).collect();
                lines.push(format!("  L{}: {} {}", d.line, d.code, msg));
                for ctx in &d.context {
                    let ctx_short: String = ctx.chars().take(120).collect();
                    lines.push(format!("    {}", ctx_short));
                }
            }
            lines.push(String::new());
        }

        lines.join("\n").trim().to_string()
    }

    fn format_ultra(&self) -> String {
        format!(
            "{} ✗{} [!]{} ({} files)",
            self.tool, self.total_errors, self.total_warnings, self.files_affected
        )
    }
}

fn truncate_str(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        format!("{}…", chars[..n].iter().collect::<String>())
    }
}

impl TokenFormatter for GhIssueList {
    fn format_compact(&self) -> String {
        let mut lines = vec!["Issues:".to_string()];
        for issue in self.issues.iter().take(20) {
            let icon = if issue.state == "OPEN" {
                "open"
            } else {
                "closed"
            };
            lines.push(format!(
                "  {} #{} {}",
                icon,
                issue.number,
                truncate_str(&issue.title, 60)
            ));
        }
        if self.issues.len() > 20 {
            lines.push(format!("  ... {} more", self.issues.len() - 20));
        }
        lines.join("\n")
    }
    fn format_verbose(&self) -> String {
        self.format_compact()
    }
    fn format_ultra(&self) -> String {
        let open = self.issues.iter().filter(|i| i.state == "OPEN").count();
        format!("Issues: {} open / {} total", open, self.issues.len())
    }
}

impl TokenFormatter for GhIssueDetail {
    fn format_compact(&self) -> String {
        let icon = if self.state == "OPEN" {
            "open"
        } else {
            "closed"
        };
        let mut lines = vec![
            format!("{} Issue #{}: {}", icon, self.number, self.title),
            format!("  Author: @{}", self.author),
            format!("  Status: {}", self.state),
            format!("  URL: {}", self.url),
        ];
        if !self.body.is_empty() {
            lines.push(String::new());
            lines.push("  Description:".to_string());
            for line in self.body.lines().take(30) {
                lines.push(format!("    {}", line));
            }
        }
        lines.join("\n")
    }
    fn format_verbose(&self) -> String {
        self.format_compact()
    }
    fn format_ultra(&self) -> String {
        let icon = if self.state == "OPEN" {
            "open"
        } else {
            "closed"
        };
        format!(
            "{} #{}: {}",
            icon,
            self.number,
            truncate_str(&self.title, 60)
        )
    }
}

impl TokenFormatter for GhRunList {
    fn format_compact(&self) -> String {
        let mut lines = vec!["Workflow Runs".to_string()];
        for run in &self.runs {
            let icon = match run.conclusion.as_deref() {
                Some("success") => "ok",
                Some("failure") => "fail",
                Some("cancelled") => "cancelled",
                _ => {
                    if run.status == "in_progress" {
                        "running"
                    } else {
                        "-"
                    }
                }
            };
            lines.push(format!(
                "  {} {} [{}]",
                icon,
                truncate_str(&run.name, 50),
                run.id
            ));
        }
        lines.join("\n")
    }
    fn format_verbose(&self) -> String {
        self.format_compact()
    }
    fn format_ultra(&self) -> String {
        let failed = self
            .runs
            .iter()
            .filter(|r| r.conclusion.as_deref() == Some("failure"))
            .count();
        let passed = self
            .runs
            .iter()
            .filter(|r| r.conclusion.as_deref() == Some("success"))
            .count();
        format!(
            "Runs: ok:{} fail:{} total:{}",
            passed,
            failed,
            self.runs.len()
        )
    }
}

impl TokenFormatter for GhRepoDetail {
    fn format_compact(&self) -> String {
        let visibility = if self.private { "Private" } else { "Public" };
        let mut lines = vec![
            format!("{}/{}", self.owner, self.name),
            format!("  {}", visibility),
        ];
        if !self.description.is_empty() {
            lines.push(format!("  {}", truncate_str(&self.description, 80)));
        }
        lines.push(format!("  {} stars | {} forks", self.stars, self.forks));
        lines.push(format!("  {}", self.url));
        lines.join("\n")
    }
    fn format_verbose(&self) -> String {
        self.format_compact()
    }
    fn format_ultra(&self) -> String {
        format!("{}/{} ({})", self.owner, self.name, self.stars)
    }
}
