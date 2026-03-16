//! CLI client for Rhizome — code intelligence via tree-sitter and LSP.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Get structured symbol list (functions, types, traits, impls) for a file.
#[allow(dead_code)]
pub fn get_symbols(file: &Path) -> Result<String> {
    run_rhizome_command("symbols", file)
}

/// Get hierarchical outline (modules, classes, methods with nesting) for a file.
pub fn get_structure(file: &Path) -> Result<String> {
    run_rhizome_command("structure", file)
}

fn run_rhizome_command(subcommand: &str, file: &Path) -> Result<String> {
    let rhizome_bin = crate::rhizome::rhizome_binary().context("Rhizome binary not found")?;

    let file_str = file.to_str().context("Invalid file path")?;

    // Spawn rhizome subprocess with timeout
    let (tx, rx) = mpsc::channel();
    let bin = rhizome_bin.to_string();
    let sub = subcommand.to_string();
    let path = file_str.to_string();

    std::thread::spawn(move || {
        let result = Command::new(&bin)
            .arg(&sub)
            .arg(&path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(result);
    });

    let output = rx
        .recv_timeout(Duration::from_secs(3))
        .context("Rhizome command timed out after 3 seconds")?
        .context("Failed to execute rhizome")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_symbols_without_rhizome() {
        // Rhizome is likely not installed in test environment
        let path = PathBuf::from("src/main.rs");
        let result = get_symbols(&path);
        // Should return Err (rhizome not found) — not panic
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
        let result = get_symbols(&path);
        // Should fail gracefully
        assert!(result.is_err());
    }
}
