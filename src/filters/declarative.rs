//! Declarative TOML-based filter declarations for mycelium.
//!
//! Simple transformations that don't require compiled Rust filters.
//! Compiled filters always take precedence when both match a command.

use serde::Deserialize;
use std::path::Path;

use crate::filter::FilterResult;

#[derive(Debug, Clone, Deserialize)]
pub struct DeclarativeFilter {
    pub filter: FilterMeta,
    #[serde(default)]
    pub transform: TransformConfig,
    #[serde(default)]
    pub truncate: TruncateConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilterMeta {
    pub name: String,
    /// Matched as a prefix against the command string (e.g. "npm test").
    pub command_pattern: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TransformConfig {
    #[serde(default)]
    pub strip_ansi: bool,
    #[serde(default)]
    pub strip_timestamps: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TruncateConfig {
    /// Keep only the last N lines (for verbose test runners).
    pub keep_last_n_lines: Option<usize>,
    /// Always keep lines matching these strings (substring match).
    #[serde(default)]
    pub keep_on_match: Vec<String>,
}

impl DeclarativeFilter {
    /// Returns true if this filter matches the given command string (prefix match).
    pub fn matches(&self, command: &str) -> bool {
        command.starts_with(&self.filter.command_pattern)
    }

    /// Apply this filter to the given input, returning a FilterResult.
    pub fn apply(&self, input: &str) -> FilterResult {
        let mut lines: Vec<&str> = input.lines().collect();

        // Transform: strip ANSI escape codes
        let stripped_ansi: Vec<String>;
        if self.transform.strip_ansi {
            stripped_ansi = lines.iter().map(|l| strip_ansi_codes(l)).collect();
            lines = stripped_ansi.iter().map(|s| s.as_str()).collect();
        }

        // Transform: strip timestamp prefixes (HH:MM:SS.mmm pattern)
        let stripped_ts: Vec<String>;
        if self.transform.strip_timestamps {
            stripped_ts = lines.iter().map(|l| strip_timestamp(l)).collect();
            lines = stripped_ts.iter().map(|s| s.as_str()).collect();
        }

        // Truncate: keep_last_n_lines + keep_on_match
        if self.truncate.keep_last_n_lines.is_some() || !self.truncate.keep_on_match.is_empty() {
            let always_keep: Vec<bool> = lines
                .iter()
                .map(|l| {
                    self.truncate
                        .keep_on_match
                        .iter()
                        .any(|pat| l.contains(pat.as_str()))
                })
                .collect();

            let n = self.truncate.keep_last_n_lines.unwrap_or(0);
            let total = lines.len();
            let last_n_start = if n > 0 && total > n { total - n } else { 0 };

            let filtered: Vec<&str> = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| *i >= last_n_start || always_keep[*i])
                .map(|(_, l)| *l)
                .collect();
            lines = filtered;
        }

        let output = lines.join("\n");
        if self.transform.strip_ansi
            || self.transform.strip_timestamps
            || self.truncate.keep_last_n_lines.is_some()
            || !self.truncate.keep_on_match.is_empty()
        {
            FilterResult::full(input, output)
        } else {
            FilterResult::degraded(input, output)
        }
    }
}

/// Load all *.toml files from a directory as declarative filters.
/// Files that fail to parse are silently skipped (logged to stderr).
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
        match toml::from_str::<DeclarativeFilter>(&contents) {
            Ok(f) => filters.push(f),
            Err(e) => eprintln!("mycelium: failed to parse {:?}: {e}", path),
        }
    }
    filters
}

/// Strip ANSI escape sequences from a string.
fn strip_ansi_codes(s: &str) -> String {
    // Match ESC [ ... m sequences
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // consume until 'm' or end
                for nc in chars.by_ref() {
                    if nc == 'm' {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Strip leading timestamp prefix like "12:34:56.789 " from a line.
fn strip_timestamp(line: &str) -> String {
    // Match HH:MM:SS.mmm pattern at line start
    let bytes = line.as_bytes();
    if bytes.len() >= 12
        && bytes[2] == b':'
        && bytes[5] == b':'
        && bytes[8] == b'.'
        && bytes[0..2].iter().all(u8::is_ascii_digit)
        && bytes[3..5].iter().all(u8::is_ascii_digit)
        && bytes[6..8].iter().all(u8::is_ascii_digit)
        && bytes[9..12].iter().all(u8::is_ascii_digit)
    {
        // Skip timestamp + optional space
        let rest = &line[12..];
        return rest.trim_start_matches(' ').to_string();
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_filter(
        pattern: &str,
        strip_ansi: bool,
        strip_ts: bool,
        keep_last: Option<usize>,
        keep_on: Vec<&str>,
    ) -> DeclarativeFilter {
        DeclarativeFilter {
            filter: FilterMeta {
                name: "test".to_string(),
                command_pattern: pattern.to_string(),
            },
            transform: TransformConfig {
                strip_ansi,
                strip_timestamps: strip_ts,
            },
            truncate: TruncateConfig {
                keep_last_n_lines: keep_last,
                keep_on_match: keep_on.into_iter().map(str::to_string).collect(),
            },
        }
    }

    #[test]
    fn matches_prefix() {
        let f = make_filter("npm test", false, false, None, vec![]);
        assert!(f.matches("npm test"));
        assert!(f.matches("npm test --watch"));
        assert!(!f.matches("npm run build"));
    }

    #[test]
    fn strip_ansi_works() {
        let f = make_filter("cmd", true, false, None, vec![]);
        let input = "\x1b[32mPASS\x1b[0m test.js";
        let result = f.apply(input);
        assert!(
            result.output.contains("PASS"),
            "expected ANSI stripped, got: {}",
            result.output
        );
        assert!(
            !result.output.contains("\x1b["),
            "ANSI codes should be gone"
        );
    }

    #[test]
    fn strip_timestamp_works() {
        let f = make_filter("cmd", false, true, None, vec![]);
        let input = "12:34:56.789 test passed\nnormal line";
        let result = f.apply(input);
        let lines: Vec<&str> = result.output.lines().collect();
        assert!(
            lines[0].starts_with("test passed"),
            "timestamp should be stripped, got: {}",
            lines[0]
        );
        assert_eq!(lines[1], "normal line");
    }

    #[test]
    fn keep_last_n_lines() {
        let f = make_filter("cmd", false, false, Some(2), vec![]);
        let input = "line1\nline2\nline3\nline4\nline5";
        let result = f.apply(input);
        let lines: Vec<&str> = result.output.lines().collect();
        assert_eq!(lines, vec!["line4", "line5"]);
    }

    #[test]
    fn keep_on_match_always_kept() {
        let f = make_filter("cmd", false, false, Some(2), vec!["PASS"]);
        let input = "PASS test1\nline2\nline3\nline4\nFAIL test5";
        let result = f.apply(input);
        assert!(
            result.output.contains("PASS test1"),
            "keep_on_match line should always appear"
        );
    }

    #[test]
    fn load_ignores_non_toml_files() {
        let dir = TempDir::new().unwrap();
        // Create a .txt file — should be ignored
        let txt_path = dir.path().join("not-a-filter.txt");
        std::fs::write(&txt_path, "not toml").unwrap();
        let filters = load_declarative_filters(dir.path());
        assert!(filters.is_empty());
    }

    #[test]
    fn load_parses_valid_toml() {
        let dir = TempDir::new().unwrap();
        let toml_content = r#"
[filter]
name = "test-filter"
command_pattern = "npm test"

[transform]
strip_ansi = true
strip_timestamps = false

[truncate]
keep_last_n_lines = 50
keep_on_match = ["PASS", "FAIL"]
"#;
        let path = dir.path().join("test.toml");
        std::fs::write(&path, toml_content).unwrap();
        let filters = load_declarative_filters(dir.path());
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].filter.name, "test-filter");
        assert!(filters[0].transform.strip_ansi);
        assert_eq!(filters[0].truncate.keep_last_n_lines, Some(50));
    }
}
