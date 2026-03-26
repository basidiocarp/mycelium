//! Multi-client MCP detection and registration.
//!
//! Detects installed MCP clients (Cursor, Windsurf, Cline, Continue, Claude Desktop,
//! Codex CLI) and registers hyphae/rhizome MCP servers in each client's config.

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Map, Value, json};
use spore::editors::{self, Editor, McpServer as EditorMcpServer};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Known MCP clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpClient {
    ClaudeCode,
    CodexCli,
    Cursor,
    Windsurf,
    Cline,
    Continue,
    ClaudeDesktop,
}

impl McpClient {
    fn shared_editor(self) -> Option<Editor> {
        match self {
            Self::ClaudeCode => Some(Editor::ClaudeCode),
            Self::CodexCli => Some(Editor::CodexCli),
            Self::Cursor => Some(Editor::Cursor),
            Self::Windsurf => Some(Editor::Windsurf),
            Self::ClaudeDesktop => Some(Editor::ClaudeDesktop),
            Self::Cline | Self::Continue => None,
        }
    }

    /// Human-readable display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::CodexCli => "Codex CLI",
            Self::Cursor => "Cursor",
            Self::Windsurf => "Windsurf",
            Self::Cline => "Cline",
            Self::Continue => "Continue",
            Self::ClaudeDesktop => "Claude Desktop",
        }
    }

    /// CLI flag value (lowercase, kebab-case). Inverse of [`from_flag`].
    #[allow(dead_code)]
    pub fn flag(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::CodexCli => "codex-cli",
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
            Self::Cline => "cline",
            Self::Continue => "continue",
            Self::ClaudeDesktop => "claude-desktop",
        }
    }

    /// Parse from CLI `--client` value.
    pub fn from_flag(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude-code" | "claude" => Some(Self::ClaudeCode),
            "codex-cli" | "codex" => Some(Self::CodexCli),
            "cursor" => Some(Self::Cursor),
            "windsurf" => Some(Self::Windsurf),
            "cline" => Some(Self::Cline),
            "continue" => Some(Self::Continue),
            "claude-desktop" => Some(Self::ClaudeDesktop),
            _ => None,
        }
    }

    /// Config file path for this client (if applicable).
    fn config_path(self) -> Option<PathBuf> {
        match self {
            Self::Cline => vscode_cline_settings_path(),
            Self::Continue => dirs::home_dir().map(|home| home.join(".continue").join("config.json")),
            _ => self
                .shared_editor()
                .and_then(|editor| editors::config_path(editor).ok()),
        }
    }
}

impl fmt::Display for McpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// All known clients in detection order.
const ALL_CLIENTS: [McpClient; 7] = [
    McpClient::ClaudeCode,
    McpClient::CodexCli,
    McpClient::Cursor,
    McpClient::Windsurf,
    McpClient::Cline,
    McpClient::Continue,
    McpClient::ClaudeDesktop,
];

/// Detect which MCP clients are installed on this system.
pub fn detect_clients() -> Vec<McpClient> {
    let shared_detected = editors::detect();
    ALL_CLIENTS
        .iter()
        .copied()
        .filter(|c| is_installed(*c, &shared_detected))
        .collect()
}

/// Detect installed host clients that can run the ecosystem's primary setup path.
pub fn detect_host_clients() -> Vec<McpClient> {
    detect_clients()
        .into_iter()
        .filter(|client| is_host_client(*client))
        .collect()
}

/// Whether the client is a primary host for ecosystem setup.
pub fn is_host_client(client: McpClient) -> bool {
    matches!(client, McpClient::ClaudeCode | McpClient::CodexCli)
}

/// Check if a client appears to be installed.
fn is_installed(client: McpClient, shared_detected: &[Editor]) -> bool {
    match client {
        McpClient::ClaudeCode => {
            Command::new("claude")
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
                || shared_editor_detected(client, shared_detected)
        }
        McpClient::CodexCli => {
            Command::new("codex")
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
                || shared_editor_detected(client, shared_detected)
        }
        McpClient::Cline => {
            vscode_cline_extension_exists() || client.config_path().is_some_and(|p| p.exists())
        }
        McpClient::Continue => client.config_path().is_some_and(|p| {
            p.exists() || p.parent().is_some_and(|d| d.exists())
        }),
        _ => shared_editor_detected(client, shared_detected),
    }
}

fn shared_editor_detected(client: McpClient, shared_detected: &[Editor]) -> bool {
    client
        .shared_editor()
        .is_some_and(|editor| shared_detected.contains(&editor))
}

/// MCP server definition for registration.
pub struct ServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Register MCP servers in a client's config.
///
/// Returns `Ok(true)` if successfully registered, `Ok(false)` if skipped.
pub fn register_servers(client: McpClient, servers: &[ServerConfig], verbose: u8) -> Result<bool> {
    match client {
        McpClient::ClaudeCode => register_claude_code(servers, verbose),
        McpClient::CodexCli
        | McpClient::Cursor
        | McpClient::Windsurf
        | McpClient::ClaudeDesktop => register_spore_editor_config(client, servers, verbose),
        McpClient::Continue => register_continue(servers, verbose),
        McpClient::Cline => {
            print_cline_snippet(servers);
            Ok(true)
        }
    }
}

/// Print a generic JSON config snippet for any MCP client.
pub fn print_generic_config(servers: &[ServerConfig]) {
    println!("{}", "Generic MCP Configuration".bold());
    println!("{}", "─".repeat(60));
    println!();
    println!("Add the following to your MCP client's config:\n");

    let mut mcp_servers = Map::new();
    for server in servers {
        mcp_servers.insert(
            server.name.clone(),
            json!({
                "command": server.command,
                "args": server.args,
            }),
        );
    }

    let config = json!({ "mcpServers": mcp_servers });
    println!(
        "{}",
        serde_json::to_string_pretty(&config).unwrap_or_default()
    );
    println!();
    println!(
        "  {}",
        "Paste this into your MCP client's settings file.".dimmed()
    );
}

// ── Claude Code ─────────────────────────────────────────────────────────────

fn register_claude_code(servers: &[ServerConfig], verbose: u8) -> Result<bool> {
    let mut all_ok = true;
    for server in servers {
        let mut cmd = Command::new("claude");
        cmd.arg("mcp")
            .arg("add")
            .arg("--scope")
            .arg("user")
            .arg(&server.name)
            .arg("--");
        cmd.arg(&server.command);
        for arg in &server.args {
            cmd.arg(arg);
        }

        if verbose > 0 {
            eprintln!(
                "  Running: claude mcp add --scope user {} -- {} {}",
                server.name,
                server.command,
                server.args.join(" ")
            );
        }

        let output = cmd.output().context("failed to run `claude mcp add`")?;
        if !output.status.success() {
            all_ok = false;
        }
    }
    Ok(all_ok)
}

fn register_spore_editor_config(
    client: McpClient,
    servers: &[ServerConfig],
    verbose: u8,
) -> Result<bool> {
    let editor = client
        .shared_editor()
        .context("client does not use shared editor registration")?;
    let config_path = editors::config_path(editor)?;
    let arg_storage: Vec<Vec<&str>> = servers
        .iter()
        .map(|server| server.args.iter().map(String::as_str).collect())
        .collect();
    let shared_servers: Vec<EditorMcpServer<'_>> = servers
        .iter()
        .zip(arg_storage.iter())
        .map(|(server, args)| EditorMcpServer {
            name: &server.name,
            command: &server.command,
            args,
        })
        .collect();

    editors::register_mcp_servers(editor, &shared_servers)
        .with_context(|| format!("failed to register MCP servers for {}", client.name()))?;

    if verbose > 0 {
        eprintln!(
            "  Wrote {} server(s) to {}",
            servers.len(),
            config_path.display()
        );
    }

    Ok(true)
}

// ── Continue ────────────────────────────────────────────────────────────────

fn register_continue(servers: &[ServerConfig], verbose: u8) -> Result<bool> {
    let config_path = McpClient::Continue
        .config_path()
        .context("no Continue config path")?;

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut root: Value = if config_path.exists() {
        let backup = config_path.with_extension("json.bak");
        fs::copy(&config_path, &backup)?;
        if verbose > 0 {
            eprintln!(
                "  Backed up {} → {}",
                config_path.display(),
                backup.display()
            );
        }
        let content = fs::read_to_string(&config_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Continue uses "experimental.modelContextProtocolServers" array
    let obj = root
        .as_object_mut()
        .context("config root is not an object")?;

    // Ensure experimental key
    let experimental = obj
        .entry("experimental")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("experimental is not an object")?;

    let mcp_array = experimental
        .entry("modelContextProtocolServers")
        .or_insert_with(|| json!([]));

    let arr = mcp_array
        .as_array_mut()
        .context("modelContextProtocolServers is not an array")?;

    for server in servers {
        // Remove existing entry with same name
        arr.retain(|entry| entry.get("name").and_then(Value::as_str) != Some(&server.name));
        arr.push(json!({
            "name": server.name,
            "command": server.command,
            "args": server.args,
        }));
    }

    let json_str = serde_json::to_string_pretty(&root)?;
    fs::write(&config_path, json_str)?;

    if verbose > 0 {
        eprintln!(
            "  Wrote {} server(s) to {}",
            servers.len(),
            config_path.display()
        );
    }

    Ok(true)
}

// ── Cline (print snippet) ──────────────────────────────────────────────────

fn print_cline_snippet(servers: &[ServerConfig]) {
    println!();
    println!(
        "  {} Cline uses VS Code settings. Add this to your VS Code settings.json:",
        "→".dimmed()
    );
    println!();

    let mut mcp_servers = Map::new();
    for server in servers {
        mcp_servers.insert(
            server.name.clone(),
            json!({
                "command": server.command,
                "args": server.args,
            }),
        );
    }

    let snippet = json!({ "cline.mcpServers": mcp_servers });
    println!(
        "{}",
        serde_json::to_string_pretty(&snippet).unwrap_or_default()
    );
    println!();
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Get the VS Code settings path where Cline stores MCP config.
fn vscode_cline_settings_path() -> Option<PathBuf> {
    editors::config_path(Editor::VsCode).ok()
}

/// Check if Cline extension directory exists in VS Code.
fn vscode_cline_extension_exists() -> bool {
    dirs::home_dir()
        .map(|h| h.join(".vscode").join("extensions"))
        .is_some_and(|ext_dir| {
            ext_dir.exists()
                && fs::read_dir(ext_dir).ok().is_some_and(|entries| {
                    entries.filter_map(Result::ok).any(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with("saoudrizwan.claude-dev")
                    })
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_flag_roundtrip() {
        for client in ALL_CLIENTS {
            let flag = client.flag();
            let parsed = McpClient::from_flag(flag);
            assert_eq!(parsed, Some(client), "roundtrip failed for {flag}");
        }
    }

    #[test]
    fn test_client_name_not_empty() {
        for client in ALL_CLIENTS {
            assert!(!client.name().is_empty());
        }
    }

    #[test]
    fn test_from_flag_aliases() {
        assert_eq!(McpClient::from_flag("claude"), Some(McpClient::ClaudeCode));
        assert_eq!(McpClient::from_flag("codex"), Some(McpClient::CodexCli));
        assert_eq!(McpClient::from_flag("CURSOR"), Some(McpClient::Cursor));
        assert_eq!(McpClient::from_flag("unknown"), None);
    }

    #[test]
    fn test_codex_config_path_shape() {
        let path = McpClient::CodexCli.config_path().unwrap();
        assert!(path.to_string_lossy().contains(".codex"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn test_shared_editor_mapping() {
        assert_eq!(McpClient::ClaudeCode.shared_editor(), Some(Editor::ClaudeCode));
        assert_eq!(McpClient::CodexCli.shared_editor(), Some(Editor::CodexCli));
        assert_eq!(McpClient::Cursor.shared_editor(), Some(Editor::Cursor));
        assert_eq!(McpClient::Windsurf.shared_editor(), Some(Editor::Windsurf));
        assert_eq!(
            McpClient::ClaudeDesktop.shared_editor(),
            Some(Editor::ClaudeDesktop)
        );
        assert_eq!(McpClient::Cline.shared_editor(), None);
        assert_eq!(McpClient::Continue.shared_editor(), None);
    }

    #[test]
    fn test_cline_settings_path_uses_vscode_config_shape() {
        let path = McpClient::Cline.config_path().unwrap();
        assert!(path.to_string_lossy().contains("Code"));
        assert!(path.to_string_lossy().ends_with("settings.json"));
    }

    #[test]
    fn test_detect_clients_does_not_panic() {
        let _clients = detect_clients();
    }

    #[test]
    fn test_print_generic_config() {
        let servers = vec![ServerConfig {
            name: "hyphae".to_string(),
            command: "hyphae".to_string(),
            args: vec!["serve".to_string()],
        }];
        // Just verify no panic
        print_generic_config(&servers);
    }
}
