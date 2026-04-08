//! CLI client for Rhizome — code intelligence via tree-sitter and LSP.

use anyhow::{Context, Result, bail};
use spore::logging::{SpanContext, subprocess_span, tool_span};
use spore::{Tool, discover};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use tracing::warn;

/// Get hierarchical outline (modules, classes, methods with nesting) for a file.
pub fn get_structure(file: &Path) -> Result<String> {
    run_rhizome_command("structure", file)
}

fn run_rhizome_command(subcommand: &str, file: &Path) -> Result<String> {
    let info = discover(Tool::Rhizome).context("Rhizome binary not found")?;
    let context = span_context(subcommand, file);
    let _tool_span = tool_span("rhizome_cli", &context).entered();

    let file_str = file.to_str().context("Invalid file path")?;

    // Spawn rhizome subprocess with timeout
    let (tx, rx) = mpsc::channel();
    let bin = info.binary_path.to_string_lossy().to_string();
    let sub = subcommand.to_string();
    let path = file_str.to_string();
    let command_label = format!("rhizome {subcommand} {file_str}");

    std::thread::spawn(move || {
        let result = Command::new(&bin)
            .arg(&sub)
            .arg(&path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(result);
    });

    let output = {
        let _subprocess_span = subprocess_span(&command_label, &context).entered();
        rx.recv_timeout(Duration::from_secs(3))
            .context("Rhizome command timed out after 3 seconds")?
            .context("Failed to execute rhizome")?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            subcommand,
            file = file_str,
            "rhizome subprocess exited non-zero: {}",
            stderr.trim()
        );
        bail!(
            "rhizome {} failed (exit {}): {}",
            subcommand,
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string();
    if stdout.is_empty() {
        bail!(
            "rhizome {} returned empty output for {}",
            subcommand,
            file_str
        );
    }

    Ok(stdout)
}

fn span_context(subcommand: &str, file: &Path) -> SpanContext {
    let context = SpanContext::for_app("mycelium").with_tool(format!("rhizome_{subcommand}"));
    match file.parent() {
        Some(parent) => context.with_workspace_root(parent.display().to_string()),
        None => context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_structure_without_rhizome() {
        let path = PathBuf::from("src/main.rs");
        let result = get_structure(&path);
        if let Err(err) = result {
            let msg = err.to_string();
            assert!(
                msg.contains("not found") || msg.contains("timed out") || msg.contains("failed"),
                "Unexpected error: {}",
                msg
            );
        }
    }

    #[test]
    fn test_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/file.rs");
        let result = get_structure(&path);
        // Should fail gracefully
        assert!(result.is_err());
    }
}
