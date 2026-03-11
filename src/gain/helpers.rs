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

/// Resolve project scope from `--project` (bool) and `--project-path <PATH>` flags.
///
/// `--project-path .` resolves `.` to the current working directory.
/// `--project` uses the current working directory directly.
/// If neither is set, returns `None` (global scope).
pub(crate) fn resolve_project_scope(
    project: bool,
    project_path: Option<&str>,
) -> Result<Option<String>> {
    if let Some(path_str) = project_path {
        let path = if path_str == "." {
            std::env::current_dir().context("Failed to resolve current working directory")?
        } else {
            std::path::PathBuf::from(path_str)
        };
        let canonical = path.canonicalize().unwrap_or(path);
        return Ok(Some(canonical.to_string_lossy().to_string()));
    }
    if !project {
        return Ok(None);
    }
    let cwd = std::env::current_dir().context("Failed to resolve current working directory")?;
    let canonical = cwd.canonicalize().unwrap_or(cwd);
    Ok(Some(canonical.to_string_lossy().to_string()))
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
