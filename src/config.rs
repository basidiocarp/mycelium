//! User configuration loaded from the platform config directory.
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
    pub compaction_profile: CompactionProfile,
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
    /// Show a header line when output is filtered (default: true)
    #[serde(default = "default_true")]
    pub show_filter_header: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompactionProfile {
    Debug,
    #[default]
    Balanced,
    Aggressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionTuning {
    pub diff_max_hunk_lines: usize,
    pub status_max_files: usize,
    pub adaptive_small_lines: usize,
    pub adaptive_small_bytes: usize,
    pub adaptive_large_lines: usize,
    /// Outputs at or below this token count pass through unfiltered (default: 500).
    pub passthrough_tokens: usize,
    /// Outputs at or below this token count use light filtering (default: 2000).
    pub light_tokens: usize,
}

impl CompactionProfile {
    pub fn tuning(self) -> CompactionTuning {
        match self {
            Self::Debug => CompactionTuning {
                diff_max_hunk_lines: 240,
                status_max_files: 150,
                adaptive_small_lines: 80,
                adaptive_small_bytes: 4096,
                adaptive_large_lines: 800,
                passthrough_tokens: 500,
                light_tokens: 2000,
            },
            Self::Balanced => CompactionTuning {
                diff_max_hunk_lines: 150,
                status_max_files: 75,
                adaptive_small_lines: 50,
                adaptive_small_bytes: 2048,
                adaptive_large_lines: 500,
                passthrough_tokens: 500,
                light_tokens: 2000,
            },
            Self::Aggressive => CompactionTuning {
                diff_max_hunk_lines: 80,
                status_max_files: 40,
                adaptive_small_lines: 25,
                adaptive_small_bytes: 1024,
                adaptive_large_lines: 250,
                passthrough_tokens: 500,
                light_tokens: 2000,
            },
        }
    }
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
            compaction_profile: CompactionProfile::default(),
            git: None,
            cargo: None,
            adaptive: None,
            hyphae: None,
            rhizome: None,
            show_filter_header: true,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;

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
        config_path()
    }

    pub fn compaction_tuning(&self) -> CompactionTuning {
        let mut tuning = self.filters.compaction_profile.tuning();
        if let Some(adaptive) = &self.filters.adaptive {
            tuning.adaptive_small_lines = adaptive.small_lines;
            tuning.adaptive_small_bytes = adaptive.small_bytes;
            tuning.adaptive_large_lines = adaptive.large_lines;
        }
        tuning
    }
}

pub fn config_path() -> Result<PathBuf> {
    crate::platform::mycelium_config_dir()
        .map(|dir| dir.join("config.toml"))
        .ok_or_else(|| anyhow::anyhow!("could not determine mycelium config directory"))
}

#[allow(
    dead_code,
    reason = "Library consumers use this through the curated lib.rs re-export"
)]
pub fn current_compaction_profile() -> CompactionProfile {
    Config::load()
        .map(|config| config.filters.compaction_profile)
        .unwrap_or_default()
}

pub fn current_compaction_tuning() -> CompactionTuning {
    Config::load()
        .map(|config| config.compaction_tuning())
        .unwrap_or_else(|_| CompactionProfile::default().tuning())
}

pub fn show_config() -> Result<()> {
    let path = config_path()?;
    let tracking_info = crate::tracking::resolve_db_path_info(None).ok();
    println!("Config: {}", path.display());
    println!();

    if path.exists() {
        let config = Config::load()?;
        println!("{}", toml::to_string_pretty(&config)?);
        println!(
            "# Effective compaction profile: {:?}",
            config.filters.compaction_profile
        );
    } else {
        println!("(default config, file not created)");
        println!();
        let config = Config::default();
        println!("{}", toml::to_string_pretty(&config)?);
        println!(
            "# Effective compaction profile: {:?}",
            config.filters.compaction_profile
        );
    }

    if let Some(info) = tracking_info {
        println!("# Tracking DB: {} ({})", info.path.display(), info.source);
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
    fn test_filter_config_default_profile() {
        let config = Config::default();
        assert_eq!(
            config.filters.compaction_profile,
            CompactionProfile::Balanced
        );
    }

    #[test]
    fn test_filter_config_profile_deserialize() {
        let toml = r#"
[filters]
compaction_profile = "debug"
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert_eq!(config.filters.compaction_profile, CompactionProfile::Debug);
        let tuning = config.compaction_tuning();
        assert_eq!(tuning.diff_max_hunk_lines, 240);
        assert_eq!(tuning.status_max_files, 150);
    }

    #[test]
    fn test_adaptive_config_overrides_profile_thresholds() {
        let toml = r#"
[filters]
compaction_profile = "aggressive"

[filters.adaptive]
small_lines = 12
small_bytes = 900
large_lines = 120
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let tuning = config.compaction_tuning();
        assert_eq!(tuning.diff_max_hunk_lines, 80);
        assert_eq!(tuning.status_max_files, 40);
        assert_eq!(tuning.adaptive_small_lines, 12);
        assert_eq!(tuning.adaptive_small_bytes, 900);
        assert_eq!(tuning.adaptive_large_lines, 120);
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

    #[test]
    fn test_hyphae_config_enabled_true() {
        let toml = r#"
[filters.hyphae]
enabled = true
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let hyphae = config.filters.hyphae.expect("hyphae config present");
        assert_eq!(hyphae.enabled, Some(true));
    }

    #[test]
    fn test_hyphae_config_enabled_false() {
        let toml = r#"
[filters.hyphae]
enabled = false
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let hyphae = config.filters.hyphae.expect("hyphae config present");
        assert_eq!(hyphae.enabled, Some(false));
    }

    #[test]
    fn test_hyphae_config_absent_is_none() {
        let config = Config::default();
        assert!(config.filters.hyphae.is_none());
    }

    #[test]
    fn test_rhizome_config_enabled_true() {
        let toml = r#"
[filters.rhizome]
enabled = true
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let rhizome = config.filters.rhizome.expect("rhizome config present");
        assert_eq!(rhizome.enabled, Some(true));
    }

    #[test]
    fn test_rhizome_config_enabled_false() {
        let toml = r#"
[filters.rhizome]
enabled = false
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        let rhizome = config.filters.rhizome.expect("rhizome config present");
        assert_eq!(rhizome.enabled, Some(false));
    }

    #[test]
    fn test_rhizome_config_absent_is_none() {
        let config = Config::default();
        assert!(config.filters.rhizome.is_none());
    }

    #[test]
    fn test_hyphae_and_rhizome_together() {
        let toml = r#"
[filters.hyphae]
enabled = true

[filters.rhizome]
enabled = false
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert_eq!(config.filters.hyphae.unwrap().enabled, Some(true));
        assert_eq!(config.filters.rhizome.unwrap().enabled, Some(false));
    }
}
