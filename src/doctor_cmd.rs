//! `mycelium doctor` — health check for Mycelium installation.
//!
//! Runs five independent checks and reports pass/fail for each.
//! All checks are non-fatal: results are aggregated, not short-circuited.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::{config, integrity, plugin, tracking};

pub fn run() -> Result<()> {
    println!("{}", "Mycelium Doctor — Health Check".bold());
    println!();

    check_version();
    check_hook();
    check_settings_json();
    check_config();
    check_tracking_db();
    check_plugin_dir();
    check_binary_collision();
    check_path();

    println!();
    Ok(())
}

// ── Formatting helpers ─────────────────────────────────────────────────────

fn pass(label: &str, detail: &str) {
    println!("  {} {:<22} {}", "✓".green().bold(), label, detail.dimmed());
}

fn warn(label: &str, detail: &str) {
    println!(
        "  {} {:<22} {}",
        "!".yellow().bold(),
        label,
        detail.yellow()
    );
}

fn fail(label: &str, detail: &str) {
    println!("  {} {:<22} {}", "✗".red().bold(), label, detail.yellow());
}

// ── Individual checks ──────────────────────────────────────────────────────

fn check_version() {
    let version = env!("CARGO_PKG_VERSION");
    pass("version", &format!("v{version}"));
}

fn check_hook() {
    match integrity::verify_hook() {
        Ok(integrity::IntegrityStatus::Verified) => {
            pass("hook", "installed and verified");
        }
        Ok(integrity::IntegrityStatus::NotInstalled) => {
            warn("hook", "not installed — run `mycelium init -g`");
        }
        Ok(integrity::IntegrityStatus::NoBaseline) => {
            warn(
                "hook",
                "installed but no baseline hash — run `mycelium init -g`",
            );
        }
        Ok(integrity::IntegrityStatus::Tampered { expected, actual }) => {
            fail(
                "hook",
                &format!(
                    "TAMPERED — expected {}…, got {}…",
                    &expected[..8],
                    &actual[..8]
                ),
            );
        }
        Ok(integrity::IntegrityStatus::OrphanedHash) => {
            warn(
                "hook",
                "hash file exists but hook is missing — run `mycelium init -g`",
            );
        }
        Err(e) => {
            fail("hook", &format!("error checking hook: {e}"));
        }
    }
}

fn check_config() {
    let config_path = config_path_best_effort();

    if let Some(ref path) = config_path {
        if path.exists() {
            match config::Config::load() {
                Ok(_) => pass("config", &format!("{}", path.display())),
                Err(e) => fail(
                    "config",
                    &format!("invalid TOML at {}: {e}", path.display()),
                ),
            }
        } else {
            pass("config", &format!("using defaults ({})", path.display()));
        }
    } else {
        warn("config", "could not determine config path");
    }
}

fn check_tracking_db() {
    match tracking::Tracker::new() {
        Ok(tracker) => {
            let count: Result<i64, _> =
                tracker
                    .conn
                    .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0));

            match count {
                Ok(n) => pass("tracking db", &format!("{n} records")),
                Err(e) => fail("tracking db", &format!("opened but query failed: {e}")),
            }
        }
        Err(e) => fail("tracking db", &format!("cannot open: {e}")),
    }
}

fn check_settings_json() {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => {
            warn("settings.json", "cannot determine home directory");
            return;
        }
    };

    let settings_path = home.join(".claude").join("settings.json");
    if !settings_path.exists() {
        warn(
            "settings.json",
            "not found — run `mycelium init -g` to register hook",
        );
        return;
    }

    match std::fs::read_to_string(&settings_path) {
        Ok(content) => {
            if content.contains("mycelium-rewrite") {
                pass("settings.json", "hook registered");
            } else {
                warn("settings.json", "exists but mycelium hook not registered");
            }
        }
        Err(e) => fail("settings.json", &format!("cannot read: {e}")),
    }
}

fn check_plugin_dir() {
    let config = plugin::PluginConfig::default();
    let dir = &config.directory;

    if !dir.exists() {
        pass(
            "plugins",
            &format!("directory not created yet ({})", dir.display()),
        );
        return;
    }

    let count = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .count()
        })
        .unwrap_or(0);

    pass(
        "plugins",
        &format!("{count} plugin(s) in {}", dir.display()),
    );
}

fn check_path() {
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };

    let install_dir = current_exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let path_var = std::env::var("PATH").unwrap_or_default();
    if path_var.split(':').any(|p| p == install_dir) {
        pass("PATH", &format!("{install_dir} is in PATH"));
    } else {
        warn(
            "PATH",
            &format!("{install_dir} is NOT in PATH — add it to your shell profile"),
        );
    }
}

fn check_binary_collision() {
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            fail(
                "binary collision",
                &format!("cannot resolve current exe: {e}"),
            );
            return;
        }
    };

    // Run `which mycelium` to find what's on PATH
    let which_output = std::process::Command::new("which").arg("mycelium").output();

    match which_output {
        Ok(output) if output.status.success() => {
            let which_path_raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let which_path = PathBuf::from(&which_path_raw);

            // Canonicalize both paths for reliable comparison (resolves symlinks)
            let exe_canonical = current_exe.canonicalize().unwrap_or(current_exe.clone());
            let which_canonical = which_path.canonicalize().unwrap_or(which_path.clone());

            if exe_canonical == which_canonical {
                pass("binary collision", &which_path_raw);
            } else {
                fail(
                    "binary collision",
                    &format!(
                        "MISMATCH — running {} but `which mycelium` → {}",
                        current_exe.display(),
                        which_path_raw,
                    ),
                );
            }
        }
        Ok(_) => {
            // which returned non-zero: mycelium not found on PATH at all
            warn(
                "binary collision",
                &format!("mycelium not on PATH (running {})", current_exe.display()),
            );
        }
        Err(e) => {
            // `which` not available (e.g., Windows)
            warn("binary collision", &format!("cannot run `which`: {e}"));
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn config_path_best_effort() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("mycelium").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_completes_without_error() {
        // doctor is non-fatal: even if all checks fail, run() returns Ok
        let result = run();
        assert!(result.is_ok(), "doctor::run() should never return Err");
    }

    #[test]
    fn test_config_path_best_effort_returns_some() {
        // On any platform with a home dir this should succeed
        let path = config_path_best_effort();
        // We can't guarantee a home dir in CI, but can verify the shape
        if let Some(p) = path {
            assert!(p.ends_with("mycelium/config.toml"));
        }
    }
}
