//! Streaming filter mode for long-running commands.
//!
//! Reads child process stdout line-by-line and applies a filter function,
//! printing matching lines immediately for real-time feedback while also
//! collecting raw output for token tracking.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Result of a streaming execution.
pub struct StreamResult {
    /// All raw output (stdout + stderr) for token tracking.
    pub raw: String,
    /// Only the lines that passed the filter, for token accounting.
    pub filtered: String,
    /// Exit code from the child process.
    pub exit_code: i32,
}

/// Execute a command with line-by-line filtering, printing matches immediately.
///
/// `filter_fn` receives each stdout line and returns:
/// - `Some(s)` → print `s` and include it in `filtered`
/// - `None`    → suppress the line
///
/// Stderr is forwarded to the caller's stderr unchanged and appended to `raw`.
///
/// # Errors
/// Returns an error if the command cannot be spawned or stdout cannot be read.
pub fn execute_streaming<F>(cmd: &str, args: &[&str], filter_fn: F) -> Result<StreamResult>
where
    F: Fn(&str) -> Option<String>,
{
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn '{}'", cmd))?;

    let stdout = child.stdout.take().context("Failed to capture stdout")?;
    let reader = BufReader::new(stdout);

    let mut raw_lines: Vec<String> = Vec::new();
    let mut filtered_lines: Vec<String> = Vec::new();
    let stdout_handle = std::io::stdout();

    for line in reader.lines() {
        let line = line.context("Failed to read line from child stdout")?;
        raw_lines.push(line.clone());
        if let Some(out) = filter_fn(&line) {
            filtered_lines.push(out.clone());
            let mut lock = stdout_handle.lock();
            writeln!(lock, "{}", out).ok();
        }
    }

    // Collect stderr and wait for the process to finish.
    let output = child
        .wait_with_output()
        .context("Failed to wait for child process")?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }

    let exit_code = output.status.code().unwrap_or(1);

    // Combine stdout lines + stderr so callers can use `raw` for token tracking.
    let raw = if stderr.is_empty() {
        raw_lines.join("\n")
    } else {
        format!("{}\n{}", raw_lines.join("\n"), stderr)
    };

    Ok(StreamResult {
        raw,
        filtered: filtered_lines.join("\n"),
        exit_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `echo` emits its argument as a single line, so use `printf` (via sh) for
    /// multi-line output.
    #[test]
    fn test_streaming_filters_lines() {
        let result =
            execute_streaming("sh", &["-c", "printf 'line1\\nline2\\nline3\\n'"], |line| {
                if line.contains('2') {
                    None
                } else {
                    Some(line.to_string())
                }
            })
            .unwrap();

        assert!(result.raw.contains("line1"), "raw should contain line1");
        assert!(result.raw.contains("line2"), "raw should contain line2");
        assert!(
            result.filtered.contains("line1"),
            "filtered should contain line1"
        );
        assert!(
            !result.filtered.contains("line2"),
            "filtered should suppress line2"
        );
        assert!(
            result.filtered.contains("line3"),
            "filtered should contain line3"
        );
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_streaming_exit_code() {
        let result =
            execute_streaming("sh", &["-c", "echo hi; exit 1"], |l| Some(l.to_string())).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.raw.contains("hi"));
    }

    #[test]
    fn test_streaming_suppress_all() {
        let result = execute_streaming("sh", &["-c", "printf 'a\\nb\\nc\\n'"], |_| None).unwrap();
        assert!(result.raw.contains('a'));
        assert!(result.filtered.is_empty(), "all lines suppressed");
        assert_eq!(result.exit_code, 0);
    }
}
