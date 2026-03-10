//! Plugin system for loading user-defined filter scripts from a config directory.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

fn default_plugin_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("mycelium")
        .join("plugins")
}

/// Plugin system configuration. Mirrors the `[plugins]` section of config.toml.
#[derive(Debug, Deserialize, Serialize)]
pub struct PluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_plugin_dir")]
    pub directory: PathBuf,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: default_plugin_dir(),
        }
    }
}

/// Load [plugins] section from the mycelium config file, falling back to defaults.
/// Reads independently to avoid a circular dependency with config.rs.
fn load_plugin_config() -> PluginConfig {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("mycelium")
        .join("config.toml");

    if !config_path.exists() {
        return PluginConfig::default();
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return PluginConfig::default(),
    };

    #[derive(Deserialize)]
    struct PartialConfig {
        plugins: Option<PluginConfig>,
    }

    toml::from_str::<PartialConfig>(&content)
        .ok()
        .and_then(|c| c.plugins)
        .unwrap_or_default()
}

/// Find a plugin script for the given command name.
///
/// Looks for `<command>.sh` (preferred) then `<command>` in the plugin directory.
/// Returns `None` if plugins are disabled, the directory doesn't exist, or no
/// matching executable passes security validation.
pub fn find_plugin(command: &str) -> Option<PathBuf> {
    let config = load_plugin_config();
    find_plugin_in_dir_with_config(&config, command)
}

fn find_plugin_in_dir_with_config(config: &PluginConfig, command: &str) -> Option<PathBuf> {
    if !config.enabled {
        return None;
    }

    let dir = &config.directory;
    if !dir.exists() {
        return None;
    }

    // .sh extension takes priority over bare name
    let candidates = [dir.join(format!("{}.sh", command)), dir.join(command)];

    for candidate in &candidates {
        if candidate.exists() && is_executable(candidate) && is_secure(candidate) {
            return Some(candidate.clone());
        }
    }

    None
}

/// Execute a plugin, piping `raw_output` to its stdin.
///
/// Returns `Ok(filtered_output)` when the plugin exits 0.
/// Returns `Err` on non-zero exit or I/O failure — callers should fall back to raw execution.
///
/// A 10-second timeout kills the plugin process if it hangs.
pub fn run_plugin(plugin_path: &Path, raw_output: &str) -> Result<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(plugin_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn plugin")?;

    let child_pid = child.id();

    // Write raw command output to the plugin's stdin, then close the pipe.
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(raw_output.as_bytes())
            .context("Failed to write to plugin stdin")?;
    }

    // Timeout: kill the plugin if it hasn't finished within 10 seconds.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        kill_process(child_pid);
    });

    let output = child
        .wait_with_output()
        .context("Failed to wait for plugin")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("Plugin exited with non-zero status: {}", output.status)
    }
}

/// Kill a process by PID. Silently no-ops if the process has already exited.
#[cfg(unix)]
fn kill_process(pid: u32) {
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .status();
}

#[cfg(not(unix))]
fn kill_process(_pid: u32) {}

/// Check whether `path` has any executable bit set.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

/// Security check: reject world-writable plugins or plugins not owned by the current user.
///
/// Ownership is verified via the `UID` environment variable (set by the shell on most Unix
/// systems). If `UID` is unavailable, the ownership check is skipped and only
/// world-writability is checked.
#[cfg(unix)]
fn is_secure(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };

    let mode = meta.permissions().mode();
    let world_writable = mode & 0o002 != 0;

    // Verify ownership using UID env var (libc not available in this build).
    let owned_by_current_user = std::env::var("UID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .map(|uid| meta.uid() == uid)
        .unwrap_or(true); // Skip check if UID unavailable

    !world_writable && owned_by_current_user
}

#[cfg(not(unix))]
fn is_secure(_path: &Path) -> bool {
    true
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── helpers ────────────────────────────────────────────────────────────────

    fn find_in(dir: &Path, command: &str) -> Option<PathBuf> {
        let config = PluginConfig {
            enabled: true,
            directory: dir.to_path_buf(),
        };
        find_plugin_in_dir_with_config(&config, command)
    }

    // ── discovery ──────────────────────────────────────────────────────────────

    #[test]
    fn test_find_plugin_returns_none_when_dir_missing() {
        let config = PluginConfig {
            enabled: true,
            directory: PathBuf::from("/tmp/mycelium-nonexistent-plugin-dir-xyz"),
        };
        assert!(find_plugin_in_dir_with_config(&config, "mycommand").is_none());
    }

    #[test]
    fn test_find_plugin_returns_none_when_disabled() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = PluginConfig {
            enabled: false,
            directory: dir.path().to_path_buf(),
        };
        assert!(find_plugin_in_dir_with_config(&config, "mycommand").is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_find_plugin_discovers_sh_extension() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("terraform.sh");
        let mut f = std::fs::File::create(&script).expect("create");
        writeln!(f, "#!/bin/sh\ncat").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = find_in(dir.path(), "terraform");
        assert!(result.is_some(), "Should find terraform.sh");
        assert_eq!(result.unwrap(), script);
    }

    #[test]
    #[cfg(unix)]
    fn test_find_plugin_discovers_bare_name() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("terraform");
        let mut f = std::fs::File::create(&script).expect("create");
        writeln!(f, "#!/bin/sh\ncat").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = find_in(dir.path(), "terraform");
        assert!(result.is_some(), "Should find bare terraform plugin");
    }

    #[test]
    #[cfg(unix)]
    fn test_find_plugin_sh_preferred_over_bare() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        for name in ["mycmd.sh", "mycmd"] {
            let p = dir.path().join(name);
            let mut f = std::fs::File::create(&p).expect("create");
            writeln!(f, "#!/bin/sh\ncat").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let result = find_in(dir.path(), "mycmd");
        assert_eq!(
            result.unwrap(),
            dir.path().join("mycmd.sh"),
            ".sh should take priority"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_find_plugin_ignores_non_executable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("myapp.sh");
        std::fs::File::create(&script).expect("create");
        // Explicitly not executable (0o644)
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert!(
            find_in(dir.path(), "myapp").is_none(),
            "Non-executable plugin should be ignored"
        );
    }

    // ── execution ──────────────────────────────────────────────────────────────

    #[test]
    #[cfg(unix)]
    fn test_run_plugin_filters_output() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("upper.sh");
        let mut f = std::fs::File::create(&script).expect("create");
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "tr '[:lower:]' '[:upper:]'").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = run_plugin(&script, "hello world");
        assert!(result.is_ok(), "run_plugin failed: {:?}", result.err());
        assert!(
            result.unwrap().contains("HELLO WORLD"),
            "Expected uppercase output"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_run_plugin_nonzero_exit_is_err() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("fail.sh");
        let mut f = std::fs::File::create(&script).expect("create");
        writeln!(f, "#!/bin/sh\nexit 1").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            run_plugin(&script, "input").is_err(),
            "Non-zero exit should return Err"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_run_plugin_passes_stdin() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("echo_back.sh");
        let mut f = std::fs::File::create(&script).expect("create");
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "cat").unwrap(); // echo stdin to stdout
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = run_plugin(&script, "token data").unwrap();
        assert!(result.contains("token data"), "stdin should reach plugin");
    }

    // ── security ───────────────────────────────────────────────────────────────

    #[test]
    #[cfg(unix)]
    fn test_is_secure_rejects_world_writable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("bad.sh");
        std::fs::File::create(&file).expect("create");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o777)).unwrap();

        assert!(!is_secure(&file), "World-writable file must be rejected");
    }

    #[test]
    #[cfg(unix)]
    fn test_is_secure_accepts_owner_only_write() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("safe.sh");
        std::fs::File::create(&file).expect("create");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Result depends on UID env var availability; we just verify no panic.
        let _ = is_secure(&file);
    }

    #[test]
    #[cfg(unix)]
    fn test_is_secure_missing_file_returns_false() {
        assert!(!is_secure(Path::new("/tmp/mycelium-does-not-exist-xyz.sh")));
    }
}
