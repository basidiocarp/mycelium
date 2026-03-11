//! `mycelium plugin` — manage user-defined filter plugins.
//!
//! Replaces `scripts/install-plugin.sh` with a built-in command. Shipped plugin
//! templates are embedded in the binary via `include_str!`, so users don't need
//! to clone the repo.

use anyhow::{Context, Result};
use colored::Colorize;

use crate::plugin::PluginConfig;

// ── Embedded plugin templates ────────────────────────────────────────────────

struct ShippedPlugin {
    name: &'static str,
    description: &'static str,
    content: &'static str,
}

const SHIPPED_PLUGINS: &[ShippedPlugin] = &[ShippedPlugin {
    name: "atmos",
    description: "Atmos infrastructure orchestration (terraform, describe, validate)",
    content: include_str!("../plugins/atmos.sh"),
}];

// ── Public API ───────────────────────────────────────────────────────────────

pub fn run_list() -> Result<()> {
    println!("{}", "Available Plugins".bold());
    println!();

    // Shipped plugins (built into binary)
    println!("  {}", "Shipped:".dimmed());
    for plugin in SHIPPED_PLUGINS {
        let installed = is_installed(plugin.name);
        let icon = if installed {
            "✓".green().to_string()
        } else {
            " ".to_string()
        };
        println!(
            "  {} {:<16} {}",
            icon,
            plugin.name,
            plugin.description.dimmed()
        );
    }

    // User plugins (already installed)
    let config = load_config();
    let dir = &config.directory;

    if dir.exists() {
        let user_plugins = discover_user_plugins(dir);
        let shipped_names: Vec<&str> = SHIPPED_PLUGINS.iter().map(|p| p.name).collect();
        let custom: Vec<_> = user_plugins
            .iter()
            .filter(|name| !shipped_names.contains(&name.as_str()))
            .collect();

        if !custom.is_empty() {
            println!();
            println!("  {}", "Custom:".dimmed());
            for name in custom {
                println!("  {} {}", "✓".green(), name);
            }
        }
    }

    println!();
    println!("  Plugin directory: {}", dir.display().to_string().dimmed());

    if !config.enabled {
        println!("  {}", "⚠ Plugins are disabled in config.toml".yellow());
    }

    Ok(())
}

pub fn run_install(name: &str, force: bool) -> Result<()> {
    let plugin = SHIPPED_PLUGINS
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            let available: Vec<&str> = SHIPPED_PLUGINS.iter().map(|p| p.name).collect();
            anyhow::anyhow!(
                "Unknown plugin: '{}'. Available: {}",
                name,
                available.join(", ")
            )
        })?;

    let config = load_config();
    let dir = &config.directory;

    // Create plugin directory if it doesn't exist
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create plugin directory: {}", dir.display()))?;

    let dest = dir.join(format!("{}.sh", name));

    if dest.exists() && !force {
        anyhow::bail!(
            "Plugin '{}' already exists at {}. Use --force to overwrite.",
            name,
            dest.display()
        );
    }

    std::fs::write(&dest, plugin.content)
        .with_context(|| format!("Failed to write plugin to {}", dest.display()))?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
            .context("Failed to set plugin permissions")?;
    }

    println!(
        "  {} Installed {} → {}",
        "✓".green(),
        name.bold(),
        dest.display()
    );

    Ok(())
}

pub fn run_install_all(force: bool) -> Result<()> {
    for plugin in SHIPPED_PLUGINS {
        run_install(plugin.name, force)?;
    }
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn load_config() -> PluginConfig {
    PluginConfig::default()
}

fn is_installed(name: &str) -> bool {
    let config = load_config();
    let dir = &config.directory;
    dir.join(format!("{name}.sh")).exists() || dir.join(name).exists()
}

fn discover_user_plugins(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && (path.extension().is_some_and(|ext| ext == "sh") || is_executable_file(&path))
        })
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".sh").unwrap_or(&name).to_string()
        })
        .collect();

    names.sort();
    names.dedup();
    names
}

#[cfg(unix)]
fn is_executable_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(_path: &std::path::Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shipped_plugins_not_empty() {
        assert!(!SHIPPED_PLUGINS.is_empty());
    }

    #[test]
    fn test_shipped_plugin_content_has_shebang() {
        for plugin in SHIPPED_PLUGINS {
            assert!(
                plugin.content.starts_with("#!/"),
                "Plugin '{}' should start with a shebang",
                plugin.name
            );
        }
    }

    #[test]
    fn test_discover_user_plugins_empty_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        let plugins = discover_user_plugins(dir.path());
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_discover_user_plugins_nonexistent_dir() {
        let plugins = discover_user_plugins(std::path::Path::new("/tmp/mycelium-nonexistent-xyz"));
        assert!(plugins.is_empty());
    }
}
