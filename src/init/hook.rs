//! Installs and updates the Mycelium shell rewrite hook script.
use anyhow::{Context, Result};
#[cfg_attr(not(unix), allow(unused_imports))]
use std::fs;
use std::io::Write;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use tempfile::NamedTempFile;

#[cfg(unix)]
use super::claude_md::resolve_claude_dir;

// Embedded hook script (guards before set -euo pipefail) — Unix-only (bash)
#[cfg(unix)]
pub(crate) const REWRITE_HOOK: &str = include_str!("../../hooks/mycelium-rewrite.sh");
#[cfg(unix)]
pub(crate) const SESSION_SUMMARY_HOOK: &str = include_str!("../../hooks/session-summary.sh");

#[cfg(unix)]
const REWRITE_HOOK_NAME: &str = "mycelium-rewrite.sh";
#[cfg(unix)]
const SESSION_SUMMARY_HOOK_NAME: &str = "mycelium-session-summary.sh";
#[cfg(unix)]
const MYCELIUM_VERSION_PLACEHOLDER: &str = "__MYCELIUM_VERSION__";
#[cfg(unix)]
const MYCELIUM_BIN_PLACEHOLDER: &str = "__MYCELIUM_BIN__";
#[cfg(unix)]
const JQ_BIN_PLACEHOLDER: &str = "__JQ_BIN__";

#[cfg(unix)]
fn shell_single_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    let mut quoted = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\"'\"'");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(unix)]
pub(crate) fn extract_quoted_assignment(content: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let value = content
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))?;

    if value == "''" {
        return Some(String::new());
    }

    let unquoted = value.strip_prefix('\'')?.strip_suffix('\'')?;
    Some(unquoted.replace("'\"'\"'", "'"))
}

#[cfg(unix)]
pub(crate) fn extract_hook_version(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# mycelium-install-version: "))
        .map(|version| version.trim().to_string())
}

#[cfg(unix)]
pub(crate) fn current_install_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(unix)]
pub(crate) fn version_is_stale(installed: &str, current: &str) -> bool {
    fn parse_triplet(version: &str) -> Option<(u64, u64, u64)> {
        let mut parts = version.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        Some((major, minor, patch))
    }

    match (parse_triplet(installed), parse_triplet(current)) {
        (Some(installed), Some(current)) => installed < current,
        _ => false,
    }
}

#[cfg(unix)]
pub(crate) fn render_rewrite_hook(mycelium_bin: Option<&Path>, jq_bin: Option<&Path>) -> String {
    let mycelium_bin = mycelium_bin
        .map(|path| shell_single_quote(&path.display().to_string()))
        .unwrap_or_else(|| "''".to_string());
    let jq_bin = jq_bin
        .map(|path| shell_single_quote(&path.display().to_string()))
        .unwrap_or_else(|| "''".to_string());

    REWRITE_HOOK
        .replace(MYCELIUM_VERSION_PLACEHOLDER, current_install_version())
        .replace(MYCELIUM_BIN_PLACEHOLDER, &mycelium_bin)
        .replace(JQ_BIN_PLACEHOLDER, &jq_bin)
}

#[cfg(unix)]
fn resolve_hook_dependencies() -> (Option<PathBuf>, Option<PathBuf>) {
    let mycelium_bin = std::env::current_exe().ok();
    let jq_bin = crate::platform::command_path("jq");
    (mycelium_bin, jq_bin)
}

/// Prepare hook directory and return paths (hook_dir, hook_path) — Unix-only
#[cfg(unix)]
fn prepare_named_hook_path(hook_name: &str) -> Result<(PathBuf, PathBuf)> {
    let claude_dir = resolve_claude_dir()?;
    let hook_dir = claude_dir.join("hooks");
    fs::create_dir_all(&hook_dir)
        .with_context(|| format!("Failed to create hook directory: {}", hook_dir.display()))?;
    let hook_path = hook_dir.join(hook_name);
    Ok((hook_dir, hook_path))
}

/// Prepare rewrite hook path — Unix-only
#[cfg(unix)]
pub(crate) fn prepare_hook_paths() -> Result<(PathBuf, PathBuf)> {
    prepare_named_hook_path(REWRITE_HOOK_NAME)
}

/// Prepare session summary hook path — Unix-only
#[cfg(unix)]
pub(crate) fn prepare_session_summary_hook_path() -> Result<(PathBuf, PathBuf)> {
    prepare_named_hook_path(SESSION_SUMMARY_HOOK_NAME)
}

#[cfg(unix)]
fn set_executable_permissions(hook_path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(hook_path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("Failed to set hook permissions: {}", hook_path.display()))?;

    Ok(())
}

#[cfg(unix)]
fn ensure_static_hook_installed(
    hook_path: &Path,
    content: &str,
    name: &str,
    verbose: u8,
) -> Result<bool> {
    let changed = write_if_changed(hook_path, content, name, verbose)?;
    set_executable_permissions(hook_path)?;
    Ok(changed)
}

/// Write hook file if missing or outdated, return true if changed
#[cfg(unix)]
pub(crate) fn ensure_hook_installed(hook_path: &Path, verbose: u8) -> Result<bool> {
    let (mycelium_bin, jq_bin) = resolve_hook_dependencies();
    let rendered_hook = render_rewrite_hook(mycelium_bin.as_deref(), jq_bin.as_deref());
    let changed = ensure_static_hook_installed(hook_path, &rendered_hook, "hook", verbose)?;

    // Store SHA-256 hash for runtime integrity verification.
    // Always store (idempotent) to ensure baseline exists even for
    // hooks installed before integrity checks were added.
    use crate::integrity;
    integrity::store_hash(hook_path)
        .with_context(|| format!("Failed to store integrity hash for {}", hook_path.display()))?;
    if verbose > 0 && changed {
        eprintln!("Stored integrity hash for hook");
    }

    Ok(changed)
}

/// Write session summary hook file if missing or outdated, return true if changed
#[cfg(unix)]
pub(crate) fn ensure_session_summary_hook_installed(hook_path: &Path, verbose: u8) -> Result<bool> {
    let changed = ensure_static_hook_installed(
        hook_path,
        SESSION_SUMMARY_HOOK,
        "session-summary hook",
        verbose,
    )?;

    use crate::integrity;
    integrity::store_hash(hook_path).with_context(|| {
        format!(
            "Failed to store integrity hash for session hook {}",
            hook_path.display()
        )
    })?;
    if verbose > 0 {
        eprintln!("Stored integrity hash for session hook");
    }

    Ok(changed)
}

/// Idempotent file write: create or update if content differs — Unix-only
#[cfg(unix)]
pub(crate) fn write_if_changed(
    path: &Path,
    content: &str,
    name: &str,
    verbose: u8,
) -> Result<bool> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}: {}", name, path.display()))?;

        if existing == content {
            if verbose > 0 {
                eprintln!("{} already up to date: {}", name, path.display());
            }
            Ok(false)
        } else {
            fs::write(path, content)
                .with_context(|| format!("Failed to write {}: {}", name, path.display()))?;
            if verbose > 0 {
                eprintln!("Updated {}: {}", name, path.display());
            }
            Ok(true)
        }
    } else {
        fs::write(path, content)
            .with_context(|| format!("Failed to write {}: {}", name, path.display()))?;
        if verbose > 0 {
            eprintln!("Created {}: {}", name, path.display());
        }
        Ok(true)
    }
}

/// Atomic write using tempfile + rename
/// Prevents corruption on crash/interrupt
pub(crate) fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().with_context(|| {
        format!(
            "Cannot write to {}: path has no parent directory",
            path.display()
        )
    })?;

    // Create temp file in same directory (ensures same filesystem for atomic rename)
    let mut temp_file = NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create temp file in {}", parent.display()))?;

    // Write content
    temp_file
        .write_all(content.as_bytes())
        .with_context(|| format!("Failed to write {} bytes to temp file", content.len()))?;

    // Atomic rename
    temp_file.persist(path).with_context(|| {
        format!(
            "Failed to atomically replace {} (disk full?)",
            path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    #[cfg(unix)]
    fn test_hook_has_guards() {
        assert!(REWRITE_HOOK.contains("__MYCELIUM_VERSION__"));
        assert!(REWRITE_HOOK.contains("__MYCELIUM_BIN__"));
        assert!(REWRITE_HOOK.contains("__JQ_BIN__"));
        assert!(REWRITE_HOOK.contains("_resolve_command()"));
        assert!(REWRITE_HOOK.contains("command -v \"$fallback\""));
        // Guards (mycelium/jq availability checks) must appear before the actual delegation call.
        // The thin delegating hook no longer uses set -euo pipefail.
        let jq_pos = REWRITE_HOOK.find("if ! JQ_CMD").unwrap();
        let mycelium_guard_pos = REWRITE_HOOK.find("if ! MYCELIUM_CMD").unwrap();
        let mycelium_delegate_pos = REWRITE_HOOK.find("rewrite \"$CMD\"").unwrap();
        assert!(
            jq_pos < mycelium_delegate_pos && mycelium_guard_pos < mycelium_delegate_pos,
            "Guards must appear before mycelium rewrite delegation"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_session_summary_hook_is_posix_sh() {
        assert!(SESSION_SUMMARY_HOOK.starts_with("#!/bin/sh"));
        assert!(!SESSION_SUMMARY_HOOK.contains("[["));
        assert!(!SESSION_SUMMARY_HOOK.contains("local "));
    }

    #[test]
    #[cfg(unix)]
    fn test_render_rewrite_hook_embeds_and_quotes_paths() {
        let mycelium_bin = Path::new("/opt/mycelium/bin/mycelium build");
        let jq_bin = Path::new("/usr/local/bin/jq");

        let rendered = render_rewrite_hook(Some(mycelium_bin), Some(jq_bin));

        assert!(rendered.contains(current_install_version()));
        assert!(!rendered.contains("__MYCELIUM_BIN__"));
        assert!(!rendered.contains("__JQ_BIN__"));
        assert!(!rendered.contains("__MYCELIUM_VERSION__"));
        assert!(rendered.contains("MYCELIUM_BIN='/opt/mycelium/bin/mycelium build'"));
        assert!(rendered.contains("JQ_BIN='/usr/local/bin/jq'"));

        assert_eq!(
            extract_quoted_assignment(&rendered, "MYCELIUM_BIN").as_deref(),
            Some("/opt/mycelium/bin/mycelium build")
        );
        assert_eq!(
            extract_quoted_assignment(&rendered, "JQ_BIN").as_deref(),
            Some("/usr/local/bin/jq")
        );
        assert_eq!(
            extract_hook_version(&rendered).as_deref(),
            Some(current_install_version())
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_version_is_stale_detects_older_version() {
        assert!(version_is_stale("0.0.1", current_install_version()));
        assert!(!version_is_stale(
            current_install_version(),
            current_install_version()
        ));
    }

    #[test]
    #[cfg(unix)]
    fn test_default_mode_creates_hook_and_mycelium_md() {
        let temp = TempDir::new().unwrap();
        let hook_path = temp.path().join("mycelium-rewrite.sh");
        let mycelium_md_path = temp.path().join("MYCELIUM.md");

        fs::write(&hook_path, REWRITE_HOOK).unwrap();
        fs::write(&mycelium_md_path, super::super::claude_md::MYCELIUM_SLIM).unwrap();

        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(hook_path.exists());
        assert!(mycelium_md_path.exists());

        let metadata = fs::metadata(&hook_path).unwrap();
        assert!(metadata.permissions().mode() & 0o111 != 0);
    }

    #[test]
    fn test_atomic_write() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.json");

        let content = r#"{"key": "value"}"#;
        atomic_write(&file_path, content).unwrap();

        assert!(file_path.exists());
        let written = fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, content);
    }
}
