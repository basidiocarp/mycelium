//! User configuration loaded from `~/.config/mycelium/config.toml`.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub filters: FilterConfig,
    #[serde(default)]
    pub tee: crate::tee::TeeConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub plugins: crate::plugin::PluginConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    /// Commands to exclude from auto-rewrite (e.g. ["curl", "playwright"]).
    /// Survives `mycelium init -g` re-runs since config.toml is user-owned.
    #[serde(default)]
    pub exclude_commands: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub enabled: bool,
    pub history_days: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_path: Option<PathBuf>,
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            history_days: 90,
            database_path: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub colors: bool,
    pub emoji: bool,
    pub max_width: usize,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            colors: true,
            emoji: true,
            max_width: 120,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_log_max_commits() -> usize {
    15
}

fn default_diff_context_lines() -> usize {
    3
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitFilterConfig {
    #[serde(default = "default_log_max_commits")]
    pub log_max_commits: usize,
    #[serde(default = "default_diff_context_lines")]
    pub diff_context_lines: usize,
    #[serde(default = "default_true")]
    pub status_show_untracked: bool,
}

impl Default for GitFilterConfig {
    fn default() -> Self {
        Self {
            log_max_commits: 15,
            diff_context_lines: 3,
            status_show_untracked: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CargoFilterConfig {
    #[serde(default)]
    pub test_show_passing: bool,
    #[serde(default = "default_true")]
    pub build_show_warnings: bool,
}

impl Default for CargoFilterConfig {
    fn default() -> Self {
        Self {
            test_show_passing: false,
            build_show_warnings: true,
        }
    }
}

/// Controls when adaptive filtering activates based on output size.
/// Small outputs pass through unfiltered; large outputs get full compression.
#[derive(Debug, Serialize, Deserialize)]
pub struct AdaptiveConfig {
    /// Outputs below this line count AND `small_bytes` pass through unfiltered (default: 50)
    pub small_lines: usize,
    /// Outputs below this byte count AND `small_lines` pass through unfiltered (default: 2048)
    pub small_bytes: usize,
    /// Outputs above this line count get full structured filtering (default: 500)
    pub large_lines: usize,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            small_lines: 50,
            small_bytes: 2048,
            large_lines: 500,
        }
    }
}

/// Controls Hyphae integration for chunked storage of large outputs.
///
/// Three modes:
/// - `enabled: None` (default) — auto-detect: use Hyphae when binary is in PATH
/// - `enabled: Some(true)` — force on: always try Hyphae (still requires binary in PATH)
/// - `enabled: Some(false)` — force off: never use Hyphae, always use local filtering
#[derive(Debug, Serialize, Deserialize)]
pub struct HyphaeConfig {
    /// Override auto-detection. `true` forces Hyphae on, `false` forces it off.
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Controls Rhizome integration for code-intelligence-enhanced file reading.
///
/// Three modes:
/// - `enabled: None` (default) — auto-detect: use Rhizome when binary is in PATH
/// - `enabled: Some(true)` — force on: always try Rhizome (still requires binary in PATH)
/// - `enabled: Some(false)` — force off: never use Rhizome, always use local filtering
#[derive(Debug, Serialize, Deserialize)]
pub struct RhizomeConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterConfig {
    #[serde(default = "default_ignore_dirs")]
    pub ignore_dirs: Vec<String>,
    #[serde(default = "default_ignore_files")]
    pub ignore_files: Vec<String>,
    #[serde(default)]
    pub git: Option<GitFilterConfig>,
    #[serde(default)]
    pub cargo: Option<CargoFilterConfig>,
    #[serde(default)]
    pub adaptive: Option<AdaptiveConfig>,
    #[serde(default)]
    pub hyphae: Option<HyphaeConfig>,
    #[serde(default)]
    pub rhizome: Option<RhizomeConfig>,
}

fn default_ignore_dirs() -> Vec<String> {
    vec![
        ".git".into(),
        "node_modules".into(),
        "target".into(),
        "__pycache__".into(),
        ".venv".into(),
        "vendor".into(),
    ]
}

fn default_ignore_files() -> Vec<String> {
    vec!["*.lock".into(), "*.min.js".into(), "*.min.css".into()]
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            ignore_dirs: default_ignore_dirs(),
            ignore_files: default_ignore_files(),
            git: None,
            cargo: None,
            adaptive: None,
            hyphae: None,
            rhizome: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = get_config_path()?;

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = get_config_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn create_default() -> Result<PathBuf> {
        let config = Config::default();
        config.save()?;
        get_config_path()
    }
}

fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    Ok(config_dir.join("mycelium").join("config.toml"))
}

pub fn show_config() -> Result<()> {
    let path = get_config_path()?;
    println!("Config: {}", path.display());
    println!();

    if path.exists() {
        let config = Config::load()?;
        println!("{}", toml::to_string_pretty(&config)?);
    } else {
        println!("(default config, file not created)");
        println!();
        let config = Config::default();
        println!("{}", toml::to_string_pretty(&config)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hooks_config_deserialize() {
        let toml = r#"
[hooks]
exclude_commands = ["curl", "gh"]
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert_eq!(config.hooks.exclude_commands, vec!["curl", "gh"]);
    }

    #[test]
    fn test_hooks_config_default_empty() {
        let config = Config::default();
        assert!(config.hooks.exclude_commands.is_empty());
    }

    #[test]
    fn test_config_without_hooks_section_is_valid() {
        let toml = r#"
[tracking]
enabled = true
history_days = 90
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert!(config.hooks.exclude_commands.is_empty());
    }

    #[test]
    fn test_git_filter_config_deserialize_custom() {
        let toml = r#"
[filters.git]
log_max_commits = 20
diff_context_lines = 5
status_show_untracked = false
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let git_config = config.filters.git.expect("git config should be present");
        assert_eq!(git_config.log_max_commits, 20);
        assert_eq!(git_config.diff_context_lines, 5);
        assert!(!git_config.status_show_untracked);
    }

    #[test]
    fn test_git_filter_config_defaults() {
        let toml = r#"
[filters.git]
log_max_commits = 25
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let git_config = config.filters.git.expect("git config should be present");
        assert_eq!(git_config.log_max_commits, 25);
        assert_eq!(git_config.diff_context_lines, 3); // default
        assert!(git_config.status_show_untracked); // default true
    }

    #[test]
    fn test_cargo_filter_config_deserialize_custom() {
        let toml = r#"
[filters.cargo]
test_show_passing = true
build_show_warnings = false
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let cargo_config = config
            .filters
            .cargo
            .expect("cargo config should be present");
        assert!(cargo_config.test_show_passing);
        assert!(!cargo_config.build_show_warnings);
    }

    #[test]
    fn test_cargo_filter_config_defaults() {
        let toml = r#"
[filters.cargo]
test_show_passing = true
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let cargo_config = config
            .filters
            .cargo
            .expect("cargo config should be present");
        assert!(cargo_config.test_show_passing);
        assert!(cargo_config.build_show_warnings); // default true
    }

    #[test]
    fn test_filter_config_without_git_cargo_sections() {
        let config = Config::default();
        assert!(config.filters.git.is_none());
        assert!(config.filters.cargo.is_none());
    }

    #[test]
    fn test_filter_config_default_git() {
        let git_config = GitFilterConfig::default();
        assert_eq!(git_config.log_max_commits, 15);
        assert_eq!(git_config.diff_context_lines, 3);
        assert!(git_config.status_show_untracked);
    }

    #[test]
    fn test_filter_config_default_cargo() {
        let cargo_config = CargoFilterConfig::default();
        assert!(!cargo_config.test_show_passing);
        assert!(cargo_config.build_show_warnings);
    }
}
