//! Shared host-adapter status helpers for `init --show` and `doctor`.
use std::path::Path;

use crate::integrity::IntegrityStatus;
use spore::editors::{self, Editor};

#[derive(Debug, Clone)]
pub(crate) struct HostAdapterStatus {
    pub name: &'static str,
    pub detected: bool,
    pub configured: bool,
    pub detail: String,
}

pub(crate) fn collect_host_adapter_statuses() -> Vec<HostAdapterStatus> {
    vec![claude_code_status(), codex_cli_status()]
}

fn claude_code_status() -> HostAdapterStatus {
    let hook_status = crate::integrity::verify_hook().unwrap_or(IntegrityStatus::NotInstalled);
    let settings_path = crate::platform::claude_settings_path();
    let settings_registered = settings_path
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|content| claude_settings_registers_mycelium(&content));
    let detected = crate::utils::which_command("claude").is_some()
        || editors::detect().contains(&Editor::ClaudeCode);

    let detail = match (&hook_status, settings_path.as_ref(), settings_registered) {
        (IntegrityStatus::Verified, Some(path), true) => {
            format!("hook verified and registered in {}", path.display())
        }
        (IntegrityStatus::NoBaseline, Some(path), true) => {
            format!(
                "hook registered in {} but missing integrity baseline",
                path.display()
            )
        }
        (IntegrityStatus::NotInstalled, Some(path), false) => {
            format!(
                "settings present at {} but Mycelium hook is not installed",
                path.display()
            )
        }
        (IntegrityStatus::Verified | IntegrityStatus::NoBaseline, Some(path), false) => {
            format!("hook installed but not registered in {}", path.display())
        }
        (IntegrityStatus::OrphanedHash, Some(path), _) => {
            format!("orphaned integrity hash; settings path {}", path.display())
        }
        (IntegrityStatus::Tampered { .. }, Some(path), _) => {
            format!("hook is tampered; settings path {}", path.display())
        }
        (_, Some(path), _) => format!("settings path {}", path.display()),
        _ => "Claude settings path not available".to_string(),
    };

    let configured = claude_adapter_configured(&hook_status, settings_registered);

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

pub(crate) fn claude_settings_registers_mycelium(content: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|root| {
            root.get("hooks")
                .and_then(|hooks| hooks.get("PreToolUse"))
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .is_some_and(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("hooks")?.as_array())
                .flatten()
                .filter_map(|hook| hook.get("command")?.as_str())
                .any(|command| command.contains("mycelium-rewrite"))
        })
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

fn claude_adapter_configured(hook_status: &IntegrityStatus, settings_registered: bool) -> bool {
    matches!(hook_status, IntegrityStatus::Verified) && settings_registered
}

pub(crate) fn claude_setup_hint() -> &'static str {
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

        assert!(claude_settings_registers_mycelium(content));
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
            true
        ));
        assert!(claude_adapter_configured(&IntegrityStatus::Verified, true));
        assert!(!claude_adapter_configured(
            &IntegrityStatus::Verified,
            false
        ));
    }
}
