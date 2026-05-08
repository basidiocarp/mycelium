//! Declarative TOML-based filter declarations for mycelium.
//!
//! Simple transformations that don't require compiled Rust filters.
//! Compiled filters always take precedence when both match a command.

use regex::Regex;
use serde::Deserialize;
use std::path::Path;

use crate::filter::FilterResult;

/// Raw TOML-deserialized filter definition.
#[derive(Debug, Clone, Deserialize)]
struct RawDeclarativeFilter {
    /// Matched as a substring of the full command string (e.g., "cargo test").
    pub command: String,
    /// Filter strategy: "truncate", "filter", "group", or "deduplicate".
    pub strategy: String,
    /// Maximum lines to keep (used by truncate strategy).
    pub max_lines: Option<usize>,
    /// Regex pattern: lines matching this are always kept.
    #[serde(default)]
    pub keep_pattern: Option<String>,
    /// Regex pattern: lines matching this are always dropped.
    #[serde(default)]
    pub drop_pattern: Option<String>,
}

/// A declarative filter with pre-compiled regex patterns.
///
/// Regex compilation happens once at load time, not on every `apply()` call.
#[derive(Debug, Clone)]
pub struct DeclarativeFilter {
    /// Matched as a substring of the full command string (e.g., "cargo test").
    pub command: String,
    /// Filter strategy: "truncate", "filter", "group", or "deduplicate".
    pub strategy: String,
    /// Maximum lines to keep (used by truncate strategy).
    pub max_lines: Option<usize>,
    /// Original keep_pattern string, retained for inspection and test assertions.
    #[allow(dead_code)]
    pub keep_pattern: Option<String>,
    /// Original drop_pattern string, retained for inspection and test assertions.
    #[allow(dead_code)]
    pub drop_pattern: Option<String>,
    /// Pre-compiled keep_pattern regex. None if no pattern or compilation failed.
    keep_regex: Option<Regex>,
    /// Pre-compiled drop_pattern regex. None if no pattern or compilation failed.
    drop_regex: Option<Regex>,
}

impl DeclarativeFilter {
    /// Construct a new filter, compiling regexes eagerly.
    /// Compilation failures are logged to stderr and stored as None.
    fn new(raw: RawDeclarativeFilter) -> Self {
        let keep_regex = raw.keep_pattern.as_deref().and_then(|p| {
            match Regex::new(p) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("mycelium: failed to compile keep_pattern regex '{p}': {e}");
                    None
                }
            }
        });

        let drop_regex = raw.drop_pattern.as_deref().and_then(|p| {
            match Regex::new(p) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("mycelium: failed to compile drop_pattern regex '{p}': {e}");
                    None
                }
            }
        });

        Self {
            command: raw.command,
            strategy: raw.strategy,
            max_lines: raw.max_lines,
            keep_pattern: raw.keep_pattern,
            drop_pattern: raw.drop_pattern,
            keep_regex,
            drop_regex,
        }
    }

    /// Construct a filter directly (used in tests), compiling regexes eagerly.
    #[cfg(test)]
    fn from_parts(
        command: &str,
        strategy: &str,
        max_lines: Option<usize>,
        keep_pattern: Option<&str>,
        drop_pattern: Option<&str>,
    ) -> Self {
        Self::new(RawDeclarativeFilter {
            command: command.to_string(),
            strategy: strategy.to_string(),
            max_lines,
            keep_pattern: keep_pattern.map(str::to_string),
            drop_pattern: drop_pattern.map(str::to_string),
        })
    }

    /// Returns true if this filter matches the given command string (substring match).
    pub fn matches(&self, command_str: &str) -> bool {
        command_str.contains(&self.command)
    }

    /// Check if a line should be kept (keep_pattern wins over drop_pattern).
    fn should_keep(&self, line: &str) -> bool {
        // Always keep lines matching keep_pattern
        if let Some(ref keep) = self.keep_regex {
            if keep.is_match(line) {
                return true;
            }
        }

        // Never keep lines matching drop_pattern (if keep_pattern didn't match)
        if let Some(ref drop) = self.drop_regex {
            if drop.is_match(line) {
                return false;
            }
        }

        true
    }

    /// Returns true if this line explicitly matches the keep_pattern.
    /// Used by strategies that need to distinguish "kept by pattern" vs "kept by default".
    fn is_keep_match(&self, line: &str) -> bool {
        match self.keep_regex {
            Some(ref keep) => keep.is_match(line),
            None => false,
        }
    }

    /// Apply this filter to the given input, returning a FilterResult.
    pub fn apply(&self, input: &str) -> FilterResult {
        let lines: Vec<&str> = input.lines().collect();

        let output = match self.strategy.as_str() {
            "truncate" => self.apply_truncate(&lines),
            "filter" => self.apply_filter(&lines),
            "group" => self.apply_group(&lines),
            "deduplicate" => self.apply_deduplicate(&lines),
            _ => {
                eprintln!("mycelium: unknown filter strategy: {}", self.strategy);
                input.to_string()
            }
        };

        if output != input {
            FilterResult::full(input, output)
        } else {
            // Output is identical to input — no transformation occurred.
            FilterResult::passthrough(input)
        }
    }

    /// Truncate strategy: keep at most max_lines lines total.
    ///
    /// Always keeps keep_pattern lines. For the remaining budget, keeps the tail
    /// of the output. If keep_pattern lines push the total above max, that is
    /// acceptable — keep_pattern always wins.
    fn apply_truncate(&self, lines: &[&str]) -> String {
        let max = self.max_lines.unwrap_or(lines.len());
        let total = lines.len();

        if total <= max {
            return lines.join("\n");
        }

        // Collect indices of keep_pattern lines outside the tail window.
        let tail_start = total - max;

        let kept: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, line)| *i >= tail_start || self.is_keep_match(line))
            .map(|(_, l)| *l)
            .collect();

        kept.join("\n")
    }

    /// Filter strategy: drop lines matching drop_pattern, always keep keep_pattern lines.
    fn apply_filter(&self, lines: &[&str]) -> String {
        lines
            .iter()
            .copied()
            .filter(|line| self.should_keep(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Group strategy: deduplicate consecutive identical lines.
    ///
    /// Lines that match keep_pattern are always emitted, even if consecutive
    /// duplicates — "always keep" overrides the dedup logic.
    fn apply_group(&self, lines: &[&str]) -> String {
        let mut result = Vec::new();
        let mut last: Option<&str> = None;

        for line in lines {
            let keep_forced = self.is_keep_match(line);

            if keep_forced {
                // keep_pattern lines are always emitted; never suppressed by dedup.
                result.push(*line);
                last = Some(*line);
            } else if self.should_keep(line) && last != Some(*line) {
                result.push(*line);
                last = Some(*line);
            }
        }

        result.join("\n")
    }

    /// Deduplicate strategy: remove any repeated lines across the whole output.
    fn apply_deduplicate(&self, lines: &[&str]) -> String {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for line in lines {
            if self.should_keep(line) && !seen.contains(*line) {
                seen.insert(line.to_string());
                result.push(*line);
            }
        }

        result.join("\n")
    }
}

/// Load all *.toml files from a directory as declarative filters.
/// Files that fail to parse are logged to stderr and skipped (do not crash).
/// If the directory does not exist, returns an empty vector.
pub fn load_declarative_filters(dir: &Path) -> Vec<DeclarativeFilter> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut filters = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            eprintln!("mycelium: failed to read declarative filter {:?}", path);
            continue;
        };
        match toml::from_str::<RawDeclarativeFilter>(&contents) {
            Ok(raw) => filters.push(DeclarativeFilter::new(raw)),
            Err(e) => {
                eprintln!("mycelium: failed to parse declarative filter {:?}: {e}", path);
            }
        }
    }
    filters
}

/// Load declarative filters from both project-local and user-global paths.
/// Project-local takes precedence for overlapping commands.
/// Returns (project_filters, user_filters) for visibility into which source matched.
pub fn load_all_declarative_filters() -> (Vec<DeclarativeFilter>, Vec<DeclarativeFilter>) {
    let mut project_filters = Vec::new();
    let mut user_filters = Vec::new();

    // Load project-local filters
    let cwd = std::env::current_dir().ok();
    if let Some(cwd) = cwd {
        let project_filter_dir = cwd.join(".mycelium/filters");
        project_filters = load_declarative_filters(&project_filter_dir);
    }

    // Load user-global filters
    if let Some(config_dir) = dirs::config_dir() {
        let user_filter_dir = config_dir.join("mycelium/filters");
        user_filters = load_declarative_filters(&user_filter_dir);
    }

    (project_filters, user_filters)
}

/// Find the first filter that matches the command string.
/// Project-local filters take precedence over user-global filters.
pub fn find_matching_filter<'a>(
    command: &str,
    project_filters: &'a [DeclarativeFilter],
    user_filters: &'a [DeclarativeFilter],
) -> Option<&'a DeclarativeFilter> {
    // Project-local filters take precedence over user-global filters.
    project_filters
        .iter()
        .find(|f| f.matches(command))
        .or_else(|| user_filters.iter().find(|f| f.matches(command)))
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_filter(
        command: &str,
        strategy: &str,
        max_lines: Option<usize>,
        keep_pattern: Option<&str>,
        drop_pattern: Option<&str>,
    ) -> DeclarativeFilter {
        DeclarativeFilter::from_parts(command, strategy, max_lines, keep_pattern, drop_pattern)
    }

    #[test]
    fn matches_substring() {
        let f = make_filter("cargo test", "truncate", None, None, None);
        assert!(f.matches("cargo test"));
        assert!(f.matches("cargo test --all"));
        assert!(f.matches("run cargo test thing"));
        assert!(!f.matches("cargo build"));
    }

    #[test]
    fn truncate_strategy_respects_max_lines() {
        let f = make_filter("test", "truncate", Some(2), None, None);
        let input = "line1\nline2\nline3\nline4\nline5";
        let result = f.apply(input);
        let lines: Vec<&str> = result.output.lines().collect();
        assert_eq!(lines, vec!["line4", "line5"]);
    }

    #[test]
    fn truncate_strategy_preserves_keep_pattern() {
        let f = make_filter("test", "truncate", Some(2), Some("IMPORTANT"), None);
        let input = "line1\nIMPORTANT\nline3\nline4\nline5";
        let result = f.apply(input);
        assert!(result.output.contains("IMPORTANT"));
    }

    #[test]
    fn truncate_keep_pattern_outside_tail_included() {
        // IMPORTANT is at index 0, tail window is lines 3..5 (max=2 out of 5)
        // Both the tail lines and IMPORTANT should appear in output.
        let f = make_filter("test", "truncate", Some(2), Some("IMPORTANT"), None);
        let input = "IMPORTANT\nline2\nline3\nline4\nline5";
        let result = f.apply(input);
        assert!(result.output.contains("IMPORTANT"), "keep_pattern line outside tail must be kept");
        assert!(result.output.contains("line4"), "tail line must be kept");
        assert!(result.output.contains("line5"), "tail line must be kept");
    }

    #[test]
    fn filter_strategy_drops_matching_lines() {
        let f = make_filter("test", "filter", None, None, Some("skip"));
        let input = "line1\nskip this\nline3";
        let result = f.apply(input);
        assert!(!result.output.contains("skip"));
        assert!(result.output.contains("line1"));
        assert!(result.output.contains("line3"));
    }

    #[test]
    fn filter_strategy_keeps_keep_pattern_lines() {
        let f = make_filter("test", "filter", None, Some("KEEP"), Some("DROP"));
        let input = "KEEP this\nDROP that\nKEEP also";
        let result = f.apply(input);
        assert!(result.output.contains("KEEP this"));
        assert!(result.output.contains("KEEP also"));
        assert!(!result.output.contains("DROP"));
    }

    #[test]
    fn group_strategy_deduplicates_consecutive() {
        let f = make_filter("test", "group", None, None, None);
        let input = "line1\nline1\nline2\nline2\nline1";
        let result = f.apply(input);
        let lines: Vec<&str> = result.output.lines().collect();
        assert_eq!(lines, vec!["line1", "line2", "line1"]);
    }

    #[test]
    fn group_strategy_does_not_dedup_keep_pattern_lines() {
        // Consecutive keep_pattern lines must both be emitted.
        let f = make_filter("test", "group", None, Some("KEEP"), None);
        let input = "KEEP\nKEEP\nother\nother";
        let result = f.apply(input);
        let lines: Vec<&str> = result.output.lines().collect();
        assert_eq!(lines, vec!["KEEP", "KEEP", "other"]);
    }

    #[test]
    fn deduplicate_strategy_removes_all_repeats() {
        let f = make_filter("test", "deduplicate", None, None, None);
        let input = "line1\nline2\nline1\nline3\nline2";
        let result = f.apply(input);
        let lines: Vec<&str> = result.output.lines().collect();
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn no_op_apply_returns_passthrough_quality() {
        use crate::filter::FilterQuality;
        // A filter with no drop_pattern and max_lines >= line count is a no-op.
        let f = make_filter("test", "filter", None, None, None);
        let input = "line1\nline2";
        let result = f.apply(input);
        assert_eq!(result.output, input);
        assert_eq!(result.quality, FilterQuality::Passthrough);
    }

    #[test]
    fn modifying_apply_returns_full_quality() {
        use crate::filter::FilterQuality;
        let f = make_filter("test", "filter", None, None, Some("drop"));
        let input = "keep\ndrop this\nkeep also";
        let result = f.apply(input);
        assert_eq!(result.quality, FilterQuality::Full);
    }

    #[test]
    fn malformed_regex_does_not_crash() {
        let f = make_filter("test", "filter", None, Some("[invalid"), None);
        let input = "line1\nline2";
        let result = f.apply(input);
        // Should not crash; may or may not apply the filter depending on regex validity
        assert!(!result.output.is_empty());
    }

    #[test]
    fn absent_filter_dir_returns_empty() {
        let nonexistent = std::path::PathBuf::from("/nonexistent/path/filters");
        let filters = load_declarative_filters(&nonexistent);
        assert!(filters.is_empty());
    }

    #[test]
    fn load_ignores_non_toml_files() {
        let dir = TempDir::new().unwrap();
        let txt_path = dir.path().join("not-a-filter.txt");
        std::fs::write(&txt_path, "not toml").unwrap();
        let filters = load_declarative_filters(dir.path());
        assert!(filters.is_empty());
    }

    #[test]
    fn load_parses_valid_toml() {
        let dir = TempDir::new().unwrap();
        let toml_content = r#"
command = "cargo test"
strategy = "truncate"
max_lines = 50
keep_pattern = "PASS|FAIL"
"#;
        let path = dir.path().join("test.toml");
        std::fs::write(&path, toml_content).unwrap();
        let filters = load_declarative_filters(dir.path());
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].command, "cargo test");
        assert_eq!(filters[0].strategy, "truncate");
        assert_eq!(filters[0].max_lines, Some(50));
        assert_eq!(filters[0].keep_pattern, Some("PASS|FAIL".to_string()));
    }

    #[test]
    fn load_malformed_toml_logs_and_skips() {
        let dir = TempDir::new().unwrap();
        let bad_toml = r#"
command = "test"
strategy = "truncate"
max_lines = "not a number"
"#;
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, bad_toml).unwrap();
        let filters = load_declarative_filters(dir.path());
        assert!(filters.is_empty());
    }

    #[test]
    fn find_matching_filter_prefers_project_local() {
        let project_filters = vec![make_filter("cargo test", "truncate", Some(10), None, None)];
        let user_filters = vec![make_filter("cargo test", "filter", None, None, None)];

        let matched = find_matching_filter("cargo test --all", &project_filters, &user_filters);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().strategy, "truncate");
    }

    #[test]
    fn find_matching_filter_falls_back_to_user() {
        let project_filters = vec![];
        let user_filters = vec![make_filter("npm test", "group", None, None, None)];

        let matched = find_matching_filter("npm test --watch", &project_filters, &user_filters);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().strategy, "group");
    }

    #[test]
    fn find_matching_filter_returns_none_on_no_match() {
        let project_filters = vec![];
        let user_filters = vec![];

        let matched = find_matching_filter("unknown command", &project_filters, &user_filters);
        assert!(matched.is_none());
    }
}
