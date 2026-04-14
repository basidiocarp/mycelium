//! Shared display helpers: styling, KPI printing, bar charts, path utilities.
use anyhow::{Context, Result};
use colored::Colorize;
use std::io::IsTerminal;
use std::path::PathBuf;

/// Format text with bold+green styling (TTY-aware).
pub(crate) fn styled(text: &str, strong: bool) -> String {
    if !std::io::stdout().is_terminal() {
        return text.to_string();
    }
    if strong {
        text.bold().green().to_string()
    } else {
        text.to_string()
    }
}

/// Print a key-value pair in KPI layout.
pub(crate) fn print_kpi(label: &str, value: String) {
    println!("{:<18} {}", format!("{label}:"), value);
}

/// Colorize percentage based on savings tier (TTY-aware).
pub(crate) fn colorize_pct_cell(pct: f64, padded: &str) -> String {
    if !std::io::stdout().is_terminal() {
        return padded.to_string();
    }
    if pct >= 70.0 {
        padded.green().bold().to_string()
    } else if pct >= 40.0 {
        padded.yellow().bold().to_string()
    } else {
        padded.red().bold().to_string()
    }
}

/// Truncate text to fit column width with ellipsis.
pub(crate) fn truncate_for_column(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let char_count = text.chars().count();
    if char_count <= width {
        return format!("{:<width$}", text, width = width);
    }
    if width <= 3 {
        return text.chars().take(width).collect();
    }
    let mut out: String = text.chars().take(width - 3).collect();
    out.push_str("...");
    out
}

/// Style command names with cyan+bold (TTY-aware).
pub(crate) fn style_command_cell(cmd: &str) -> String {
    if !std::io::stdout().is_terminal() {
        return cmd.to_string();
    }
    cmd.bright_cyan().bold().to_string()
}

/// Render a proportional bar chart segment (TTY-aware).
pub(crate) fn mini_bar(value: usize, max: usize, width: usize) -> String {
    if max == 0 || width == 0 {
        return String::new();
    }
    let filled = ((value as f64 / max as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut bar = "█".repeat(filled);
    bar.push_str(&"░".repeat(width - filled));
    if std::io::stdout().is_terminal() {
        bar.cyan().to_string()
    } else {
        bar
    }
}

/// Print an efficiency meter with colored progress bar (TTY-aware).
pub(crate) fn print_efficiency_meter(pct: f64) {
    let width = 24usize;
    let filled = (((pct / 100.0) * width as f64).round() as usize).min(width);
    let meter = format!("{}{}", "█".repeat(filled), "░".repeat(width - filled));
    if std::io::stdout().is_terminal() {
        let pct_str = format!("{pct:.1}%");
        let colored_pct = if pct >= 70.0 {
            pct_str.green().bold().to_string()
        } else if pct >= 40.0 {
            pct_str.yellow().bold().to_string()
        } else {
            pct_str.red().bold().to_string()
        };
        println!("Efficiency meter: {} {}", meter.green(), colored_pct);
    } else {
        println!("Efficiency meter: {} {:.1}%", meter, pct);
    }
}

/// Resolve project scope from `--project [NAME]` and `--project-path <PATH>` flags.
///
/// `--project-path .` resolves `.` to the current working directory.
/// `--project` (bare, no value) uses the current working directory.
/// `--project <name>` searches known project paths for a substring match.
/// If neither is set, returns `None` (global scope).
pub(crate) fn resolve_project_scope(
    project: Option<&str>,
    project_path: Option<&str>,
    tracker: &crate::tracking::Tracker,
) -> Result<Option<String>> {
    if let Some(path_str) = project_path {
        let path = if path_str == "." {
            std::env::current_dir().context("Failed to resolve current working directory")?
        } else {
            std::path::PathBuf::from(path_str)
        };
        let canonical = path.canonicalize().unwrap_or_else(|e| {
            tracing::debug!("Failed to canonicalize {:?}: {e}", path);
            path
        });
        return Ok(Some(canonical.to_string_lossy().to_string()));
    }
    match project {
        None => Ok(None),
        // "all" is a reserved keyword handled by the caller (gain::run) before
        // reaching this helper. Reject it here defensively so a direct caller
        // does not accidentally substring-match paths containing "all".
        Some(s) if s.eq_ignore_ascii_case("all") => {
            anyhow::bail!(
                "'all' is a reserved keyword for per-project breakdown. \
                 Use `mycelium gain --project all` to show the per-project table."
            )
        }
        // Bare --project (default_missing_value = ".")
        Some(".") => {
            let cwd =
                std::env::current_dir().context("Failed to resolve current working directory")?;
            let canonical = cwd.canonicalize().unwrap_or_else(|e| {
                tracing::debug!("Failed to canonicalize {:?}: {e}", cwd);
                cwd
            });
            Ok(Some(canonical.to_string_lossy().to_string()))
        }
        // Named substring match — "all" is caught above
        Some(name) => {
            // Search known projects for a matching name (case-insensitive substring)
            let projects = tracker
                .get_by_project()
                .context("Failed to load project list for name lookup")?;
            let needle = name.to_ascii_lowercase();
            let matches: Vec<_> = projects
                .iter()
                .filter(|p| p.project_path.to_ascii_lowercase().contains(&needle))
                .collect();

            match matches.len() {
                0 => {
                    // Try as a literal path before giving up
                    let path = std::path::PathBuf::from(name);
                    if path.exists() {
                        let canonical = path.canonicalize().unwrap_or_else(|e| {
                            tracing::debug!("Failed to canonicalize {:?}: {e}", path);
                            path
                        });
                        Ok(Some(canonical.to_string_lossy().to_string()))
                    } else {
                        anyhow::bail!(
                            "No project found matching '{}'. Use `mycelium gain --project all` to list known projects.",
                            name
                        );
                    }
                }
                1 => Ok(Some(matches[0].project_path.clone())),
                _ => {
                    let mut msg = format!(
                        "Multiple projects match '{}'. Be more specific or use the full path:\n",
                        name
                    );
                    for m in &matches {
                        msg.push_str(&format!("  {}\n", m.project_path));
                    }
                    anyhow::bail!(msg);
                }
            }
        }
    }
}

/// Shorten long absolute paths for display.
pub(crate) fn shorten_path(path: &str) -> String {
    use std::path::{Component, MAIN_SEPARATOR};
    let path_buf = PathBuf::from(path);
    let comps: Vec<_> = path_buf.components().collect();
    if comps.len() <= 4 {
        return path.to_string();
    }
    let tail_2 = comps[comps.len() - 2].as_os_str().to_string_lossy();
    let tail_1 = comps[comps.len() - 1].as_os_str().to_string_lossy();
    let sep = MAIN_SEPARATOR;
    match comps[0] {
        Component::RootDir => format!("{sep}...{sep}{tail_2}{sep}{tail_1}"),
        Component::Prefix(_) => {
            let prefix = comps[0].as_os_str().to_string_lossy();
            format!("{prefix}{sep}...{sep}{tail_2}{sep}{tail_1}")
        }
        _ => format!("...{sep}{tail_2}{sep}{tail_1}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::Tracker;
    use tempfile::tempdir;

    fn test_tracker(name: &str) -> (Tracker, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join(format!("{name}.db"));
        let tracker =
            Tracker::new_with_override(Some(&db_path.to_string_lossy())).expect("tracker");
        (tracker, dir)
    }

    #[test]
    fn test_resolve_project_scope_none_returns_none() {
        let (tracker, _dir) = test_tracker("resolve_none");
        let result = resolve_project_scope(None, None, &tracker).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_project_scope_all_is_rejected() {
        let (tracker, _dir) = test_tracker("resolve_all");
        for keyword in &["all", "ALL", "All"] {
            let result = resolve_project_scope(Some(keyword), None, &tracker);
            assert!(result.is_err(), "'{}' should be rejected", keyword);
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("reserved keyword"),
                "error for '{}' should mention reserved keyword, got: {}",
                keyword,
                msg
            );
        }
    }

    #[test]
    fn test_resolve_project_scope_bare_uses_cwd() {
        let (tracker, _dir) = test_tracker("resolve_bare");
        let result = resolve_project_scope(Some("."), None, &tracker).unwrap();
        assert!(result.is_some());
        let cwd = std::env::current_dir()
            .unwrap()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(result.unwrap(), cwd);
    }

    #[test]
    fn test_resolve_project_scope_name_matches_single() {
        let (tracker, _dir) = test_tracker("resolve_name");
        // Seed a project row
        tracker
            .conn
            .execute(
                "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, \
                 input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    "2026-04-01T12:00:00Z",
                    "git status",
                    "mycelium git status",
                    "/tmp/test-project-alpha",
                    100,
                    20,
                    80,
                    80.0,
                    5,
                ],
            )
            .unwrap();

        let result = resolve_project_scope(Some("alpha"), None, &tracker).unwrap();
        assert_eq!(result.as_deref(), Some("/tmp/test-project-alpha"));
    }

    #[test]
    fn test_resolve_project_scope_name_no_match_errors() {
        let (tracker, _dir) = test_tracker("resolve_no_match");
        let result = resolve_project_scope(Some("nonexistent"), None, &tracker);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No project found matching"));
    }

    #[test]
    fn test_resolve_project_scope_name_ambiguous_errors() {
        let (tracker, _dir) = test_tracker("resolve_ambiguous");
        for path in &["/tmp/my-project-a", "/tmp/my-project-b"] {
            tracker
                .conn
                .execute(
                    "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, \
                     input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        "2026-04-01T12:00:00Z",
                        "git status",
                        "mycelium git status",
                        path,
                        100,
                        20,
                        80,
                        80.0,
                        5,
                    ],
                )
                .unwrap();
        }

        let result = resolve_project_scope(Some("my-project"), None, &tracker);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Multiple projects match"));
    }

    #[test]
    fn test_resolve_project_scope_project_path_takes_precedence() {
        let (tracker, _dir) = test_tracker("resolve_path_precedence");
        let dir = tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        let result = resolve_project_scope(Some("anything"), Some(&path), &tracker).unwrap();
        assert!(result.is_some());
        // project_path takes precedence over project name
        let expected = dir
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_truncate_for_column_short_text() {
        let result = truncate_for_column("git status", 24);
        assert_eq!(result, "git status              ");
    }

    #[test]
    fn test_truncate_for_column_exact_width() {
        let result = truncate_for_column("abc", 3);
        assert_eq!(result, "abc");
    }

    #[test]
    fn test_truncate_for_column_long_text() {
        let result = truncate_for_column("very-long-command-name-here", 10);
        assert_eq!(result, "very-lo...");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn test_truncate_for_column_zero_width() {
        let result = truncate_for_column("anything", 0);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_for_column_width_le_3() {
        let result = truncate_for_column("abcdef", 2);
        assert_eq!(result, "ab");
    }

    #[test]
    #[cfg(unix)]
    fn test_shorten_path_short() {
        // 3 components: / + usr + bin → should return as-is
        let result = shorten_path("/usr/bin");
        assert_eq!(result, "/usr/bin");
    }

    #[test]
    #[cfg(unix)]
    fn test_shorten_path_long() {
        let result = shorten_path("/home/user/projects/myapp/src");
        assert_eq!(result, "/.../myapp/src");
    }

    #[test]
    #[cfg(windows)]
    fn test_shorten_path_long_windows() {
        let result = shorten_path(r"C:\Users\user\projects\myapp\src");
        assert_eq!(result, r"C:\...\myapp\src");
    }

    #[test]
    fn test_mini_bar_full() {
        let result = mini_bar(100, 100, 10);
        // Non-TTY: plain text without color
        assert!(result.contains('█'));
        // All filled when value == max
        let filled_count = result.matches('█').count();
        assert_eq!(filled_count, 10);
    }

    #[test]
    fn test_mini_bar_half() {
        let result = mini_bar(50, 100, 10);
        let filled = result.matches('█').count();
        let empty = result.matches('░').count();
        assert_eq!(filled, 5);
        assert_eq!(empty, 5);
    }

    #[test]
    fn test_mini_bar_zero_max() {
        let result = mini_bar(50, 0, 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_mini_bar_zero_width() {
        let result = mini_bar(50, 100, 0);
        assert_eq!(result, "");
    }
}
