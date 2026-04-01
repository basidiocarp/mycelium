//! Shared host-adapter status helpers for `init --show` and `doctor`.
use std::path::Path;

use super::claude_md::resolve_claude_dir;
use crate::integrity::IntegrityStatus;
use spore::editors::{self, Editor};

const LEGACY_MYCELIUM_HOOK_COMMAND: &str = "mycelium-rewrite";
const CORTINA_PRE_TOOL_USE_COMMAND: &str = "cortina adapter claude-code pre-tool-use";

#[derive(Debug, Clone, Copy)]
pub(crate) struct HostCapability {
    pub supported: bool,
    pub detail: &'static str,
}

impl HostCapability {
    const fn supported(detail: &'static str) -> Self {
        Self {
            supported: true,
            detail,
        }
    }

    const fn unsupported(detail: &'static str) -> Self {
        Self {
            supported: false,
            detail,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ClaudeCodeCapabilities {
    pub hook_adapter: HostCapability,
    pub settings_patch: HostCapability,
    pub slim_global_setup: HostCapability,
    pub legacy_claude_md: HostCapability,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaudeMdStatus {
    pub configured: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub(crate) struct HostAdapterStatus {
    pub name: &'static str,
    pub detected: bool,
    pub configured: bool,
    pub detail: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaudeHookRegistration {
    pub legacy_mycelium: bool,
    pub cortina_pre_tool_use: bool,
}

pub(crate) fn claude_code_capabilities() -> ClaudeCodeCapabilities {
    if cfg!(unix) {
        ClaudeCodeCapabilities {
            hook_adapter: HostCapability::supported(
                "supported: Mycelium can install the Claude Code Bash hook adapter on this platform",
            ),
            settings_patch: HostCapability::supported(
                "supported: Mycelium can patch Claude settings.json for the hook adapter on this platform",
            ),
            slim_global_setup: HostCapability::supported(
                "supported: global MYCELIUM.md + @MYCELIUM.md slim setup can be managed alongside the hook adapter",
            ),
            legacy_claude_md: HostCapability::supported(
                "supported: legacy CLAUDE.md-only injection is available",
            ),
        }
    } else {
        ClaudeCodeCapabilities {
            hook_adapter: HostCapability::unsupported(
                "unsupported on this platform: the Claude Code hook adapter currently depends on the Bash thin delegator used on macOS/Linux; use `mycelium init -g --claude-md` for docs-only global setup",
            ),
            settings_patch: HostCapability::unsupported(
                "unsupported on this platform: settings.json patching is tied to the Bash hook adapter flow",
            ),
            slim_global_setup: HostCapability::unsupported(
                "unsupported on this platform: the global MYCELIUM.md + @MYCELIUM.md slim setup is bundled with the Bash hook adapter flow; use `mycelium init -g --claude-md` for docs-only global setup",
            ),
            legacy_claude_md: HostCapability::supported(
                "supported: legacy CLAUDE.md-only injection is available",
            ),
        }
    }
}

pub(crate) fn collect_host_adapter_statuses() -> Vec<HostAdapterStatus> {
    vec![claude_code_status(), codex_cli_status()]
}

fn claude_code_status() -> HostAdapterStatus {
    let capabilities = claude_code_capabilities();
    let hook_status = crate::integrity::verify_hook().unwrap_or(IntegrityStatus::NotInstalled);
    let settings_path = crate::platform::claude_settings_path();
    let settings_registration = settings_path
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|content| claude_settings_hook_registration(&content))
        .unwrap_or_default();
    let detected = crate::utils::which_command("claude").is_some()
        || editors::detect().contains(&Editor::ClaudeCode);

    if !capabilities.hook_adapter.supported {
        let docs_status = global_claude_md_status();
        return HostAdapterStatus {
            name: "Claude Code",
            detected,
            configured: docs_status.configured,
            detail: if docs_status.configured {
                docs_status.detail
            } else {
                format!(
                    "{}; {}",
                    capabilities.hook_adapter.detail, docs_status.detail
                )
            },
        };
    }

    let detail = match (
        &hook_status,
        settings_path.as_ref(),
        settings_registration.cortina_pre_tool_use,
        settings_registration.legacy_mycelium,
    ) {
        (_, Some(path), true, _) => {
            format!("Cortina PreToolUse hook registered in {}", path.display())
        }
        (IntegrityStatus::Verified, Some(path), false, true) => {
            format!("hook verified and registered in {}", path.display())
        }
        (IntegrityStatus::NoBaseline, Some(path), false, true) => {
            format!(
                "hook registered in {} but missing integrity baseline",
                path.display()
            )
        }
        (IntegrityStatus::NotInstalled, Some(path), false, true) => {
            format!(
                "legacy hook registered in {} but Mycelium hook is not installed",
                path.display()
            )
        }
        (IntegrityStatus::NotInstalled, Some(path), false, false) => {
            format!(
                "settings present at {} but Mycelium hook is not installed",
                path.display()
            )
        }
        (IntegrityStatus::Verified | IntegrityStatus::NoBaseline, Some(path), false, false) => {
            format!("hook installed but not registered in {}", path.display())
        }
        (IntegrityStatus::OrphanedHash, Some(path), false, _) => {
            format!("orphaned integrity hash; settings path {}", path.display())
        }
        (IntegrityStatus::Tampered { .. }, Some(path), false, _) => {
            format!("hook is tampered; settings path {}", path.display())
        }
        _ => "Claude settings path not available".to_string(),
    };

    let configured = claude_adapter_configured(&hook_status, settings_registration);

    HostAdapterStatus {
        name: "Claude Code",
        detected,
        configured,
        detail,
    }
}

fn codex_cli_status() -> HostAdapterStatus {
    let path = editors::config_path(Editor::CodexCli).ok();
    let servers = path
        .as_ref()
        .and_then(|config_path| codex_registered_servers_at_path(config_path).ok())
        .unwrap_or_default();
    let detected = crate::utils::which_command("codex").is_some()
        || editors::detect().contains(&Editor::CodexCli);

    let detail = match (&path, servers.is_empty()) {
        (Some(config_path), false) => format!(
            "configured in {} ({})",
            config_path.display(),
            servers.join(", ")
        ),
        (Some(config_path), true) if config_path.exists() => {
            format!(
                "config exists at {} but no Basidiocarp MCP servers are registered",
                config_path.display()
            )
        }
        (Some(config_path), true) => format!("config not found at {}", config_path.display()),
        (None, _) => "Codex config path not available".to_string(),
    };

    HostAdapterStatus {
        name: "Codex CLI",
        detected,
        configured: !servers.is_empty(),
        detail,
    }
}

pub(crate) fn claude_settings_hook_registration(content: &str) -> ClaudeHookRegistration {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|root| {
            root.get("hooks")
                .and_then(|hooks| hooks.get("PreToolUse"))
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("hooks")?.as_array())
                .flatten()
                .filter_map(|hook| hook.get("command")?.as_str())
                .fold(
                    ClaudeHookRegistration::default(),
                    |mut registration, command| {
                        if command.contains(LEGACY_MYCELIUM_HOOK_COMMAND) {
                            registration.legacy_mycelium = true;
                        }
                        if command.contains(CORTINA_PRE_TOOL_USE_COMMAND) {
                            registration.cortina_pre_tool_use = true;
                        }
                        registration
                    },
                )
        })
        .unwrap_or_default()
}

pub(crate) fn codex_registered_servers_at_path(path: &Path) -> anyhow::Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)?;
    codex_registered_servers_from_str(&content)
}

pub(crate) fn codex_registered_servers_from_str(content: &str) -> anyhow::Result<Vec<String>> {
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let root: toml::Value = toml::from_str(content)?;
    let mut servers = root
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .map(|table| {
            table
                .keys()
                .filter(|name| matches!(name.as_str(), "hyphae" | "rhizome"))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    servers.sort();
    Ok(servers)
}

fn claude_adapter_configured(
    hook_status: &IntegrityStatus,
    settings_registration: ClaudeHookRegistration,
) -> bool {
    settings_registration.cortina_pre_tool_use
        || (matches!(hook_status, IntegrityStatus::Verified)
            && settings_registration.legacy_mycelium)
}

pub(crate) fn global_claude_md_status() -> ClaudeMdStatus {
    let path = match resolve_claude_dir() {
        Ok(dir) => dir.join("CLAUDE.md"),
        Err(_) => {
            return ClaudeMdStatus {
                configured: false,
                detail: "Claude home directory is not available".to_string(),
            };
        }
    };

    if !path.exists() {
        return ClaudeMdStatus {
            configured: false,
            detail: format!("docs-only fallback not configured in {}", path.display()),
        };
    }

    match std::fs::read_to_string(&path) {
        Ok(content) if content.contains("@MYCELIUM.md") => ClaudeMdStatus {
            configured: true,
            detail: format!("slim global setup configured in {}", path.display()),
        },
        Ok(content) if content.contains("<!-- mycelium-instructions") => ClaudeMdStatus {
            configured: true,
            detail: format!("legacy docs-only fallback configured in {}", path.display()),
        },
        Ok(_) => ClaudeMdStatus {
            configured: false,
            detail: format!(
                "CLAUDE.md exists at {} but Mycelium is not configured",
                path.display()
            ),
        },
        Err(error) => ClaudeMdStatus {
            configured: false,
            detail: format!("cannot read {}: {error}", path.display()),
        },
    }
}

pub(crate) fn claude_setup_hint() -> &'static str {
    let capabilities = claude_code_capabilities();
    if !capabilities.hook_adapter.supported {
        return "mycelium init -g --claude-md";
    }
    if crate::utils::which_command("stipe").is_some() {
        "stipe init"
    } else {
        "mycelium init -g"
    }
}

pub(crate) fn codex_setup_hint() -> &'static str {
    if crate::utils::which_command("stipe").is_some() {
        "stipe init"
    } else {
        "install stipe, then run stipe init"
    }
}

pub(crate) fn operator_setup_hint() -> &'static str {
    claude_setup_hint()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_settings_registers_mycelium() {
        let content = r#"{
          "hooks": {
            "PreToolUse": [{
              "matcher": "Bash",
              "hooks": [{
                "type": "command",
                "command": "/Users/test/.claude/hooks/mycelium-rewrite.sh"
              }]
            }]
          }
        }"#;

        let registration = claude_settings_hook_registration(content);
        assert!(registration.legacy_mycelium || registration.cortina_pre_tool_use);
    }

    #[test]
    fn test_claude_settings_registers_cortina_pre_tool_use() {
        let content = r#"{
          "hooks": {
            "PreToolUse": [{
              "matcher": "Bash",
              "hooks": [{
                "type": "command",
                "command": "cortina adapter claude-code pre-tool-use"
              }]
            }]
          }
        }"#;

        assert_eq!(
            claude_settings_hook_registration(content),
            ClaudeHookRegistration {
                legacy_mycelium: false,
                cortina_pre_tool_use: true,
            }
        );
        let registration = claude_settings_hook_registration(content);
        assert!(registration.legacy_mycelium || registration.cortina_pre_tool_use);
    }

    #[test]
    fn test_codex_registered_servers_from_str_detects_basidiocarp_servers() {
        let content = r#"
            [mcp_servers.hyphae]
            command = "hyphae"
            args = ["serve"]

            [mcp_servers.rhizome]
            command = "rhizome"
            args = ["serve", "--expanded"]
        "#;

        let servers = codex_registered_servers_from_str(content).unwrap();
        assert_eq!(servers, vec!["hyphae".to_string(), "rhizome".to_string()]);
    }

    #[test]
    fn test_no_baseline_does_not_count_as_configured() {
        assert!(!claude_adapter_configured(
            &IntegrityStatus::NoBaseline,
            ClaudeHookRegistration {
                legacy_mycelium: true,
                cortina_pre_tool_use: false,
            }
        ));
        assert!(claude_adapter_configured(
            &IntegrityStatus::Verified,
            ClaudeHookRegistration {
                legacy_mycelium: true,
                cortina_pre_tool_use: false,
            }
        ));
        assert!(!claude_adapter_configured(
            &IntegrityStatus::Verified,
            ClaudeHookRegistration::default()
        ));
    }

    #[test]
    fn test_cortina_pre_tool_use_counts_as_configured_without_legacy_hook() {
        assert!(claude_adapter_configured(
            &IntegrityStatus::NotInstalled,
            ClaudeHookRegistration {
                legacy_mycelium: false,
                cortina_pre_tool_use: true,
            }
        ));
    }

    #[test]
    fn test_legacy_claude_md_mode_is_always_supported() {
        let capabilities = claude_code_capabilities();
        assert!(capabilities.legacy_claude_md.supported);
    }

    #[test]
    fn test_global_claude_md_status_reports_missing_file() {
        let status = global_claude_md_status();
        if !status.configured {
            assert!(!status.detail.is_empty());
        }
    }
}
