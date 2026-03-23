//! `mycelium init --ecosystem` — detect sibling tools and configure host clients.

use anyhow::Result;
use colored::Colorize;
use spore::{Tool, discover};
use std::process::Command;

use super::clients::{self, McpClient, ServerConfig};

/// Cap is not in the spore `Tool` enum — detect it separately.
fn discover_cap() -> Option<String> {
    let output = Command::new("cap").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().last())
        .filter(|v| v.contains('.'))
        .unwrap_or("unknown")
        .to_string();
    Some(version)
}

/// Main entry point for `mycelium init --ecosystem`.
pub fn run_ecosystem(client: Option<&str>, verbose: u8) -> Result<()> {
    // Handle --client generic: just print JSON snippet and exit
    if client
        .as_ref()
        .is_some_and(|c| c.eq_ignore_ascii_case("generic"))
    {
        let servers = build_server_configs();
        clients::print_generic_config(&servers);
        return Ok(());
    }

    // ── 1. Discover tools ──────────────────────────────────────────────────
    let cap_version = discover_cap();

    // ── 2. Print ecosystem status ──────────────────────────────────────────
    println!();
    println!("{}", "Basidiocarp Ecosystem Status".bold());
    println!("{}", "\u{2500}".repeat(75));
    println!();

    // Always show mycelium first (we know it's installed — we're running it)
    let mycelium_info = discover(Tool::Mycelium);
    print_tool_status(
        "mycelium",
        mycelium_info.as_ref().map(|i| i.version.as_str()),
    );

    let hyphae_info = discover(Tool::Hyphae);
    print_tool_status("hyphae", hyphae_info.as_ref().map(|i| i.version.as_str()));

    let rhizome_info = discover(Tool::Rhizome);
    print_tool_status("rhizome", rhizome_info.as_ref().map(|i| i.version.as_str()));

    print_tool_status("cap", cap_version.as_deref());

    println!();

    // ── 3. Configure detected MCP clients ──────────────────────────────────
    configure_detected_clients(client, &hyphae_info, &rhizome_info, verbose);

    // ── 4. Print missing tool instructions ─────────────────────────────────
    let mut missing: Vec<(&str, &str)> = Vec::new();

    if hyphae_info.is_none() {
        missing.push((
            "hyphae",
            "cargo install --git https://github.com/basidiocarp/hyphae hyphae-cli --no-default-features",
        ));
    }
    if rhizome_info.is_none() {
        missing.push((
            "rhizome",
            "cargo install --git https://github.com/basidiocarp/rhizome rhizome-cli",
        ));
    }
    if cap_version.is_none() {
        missing.push((
            "cap",
            "git clone https://github.com/basidiocarp/cap && cd cap && npm i && npm run dev:all",
        ));
    }

    if !missing.is_empty() {
        println!();
        println!("{}", "Missing tools:".bold());
        for (name, cmd) in &missing {
            println!("  {:<10}{} {}", name, "\u{2192}".dimmed(), cmd.dimmed());
        }
        println!();
        println!(
            "Or install all: {}",
            "curl -sSfL https://raw.githubusercontent.com/basidiocarp/.github/main/install.sh | sh"
                .dimmed()
        );
    }

    println!();
    Ok(())
}

/// Print a single tool's status line.
fn print_tool_status(name: &str, version: Option<&str>) {
    match version {
        Some(v) => {
            println!(
                "  {:<10}v{:<8}{}",
                name.bold(),
                v,
                "\u{2713} installed".green()
            );
        }
        None => {
            let hint = match name {
                "cap" => {
                    " (optional: git clone https://github.com/basidiocarp/cap && cd cap && npm i && npm run dev:all)"
                }
                _ => "",
            };
            println!(
                "  {:<10}{:<8} {}{}",
                name.bold(),
                "\u{2014}",
                "\u{2717} not installed".red(),
                hint.dimmed()
            );
        }
    }
}

/// Build MCP server configurations from discovered tools.
fn build_server_configs() -> Vec<ServerConfig> {
    let mut servers = Vec::new();
    if discover(Tool::Hyphae).is_some() {
        servers.push(ServerConfig {
            name: "hyphae".to_string(),
            command: "hyphae".to_string(),
            args: vec!["serve".to_string()],
        });
    }
    if discover(Tool::Rhizome).is_some() {
        servers.push(ServerConfig {
            name: "rhizome".to_string(),
            command: "rhizome".to_string(),
            args: vec!["serve".to_string(), "--expanded".to_string()],
        });
    }
    servers
}

/// Configure detected MCP clients.
fn configure_detected_clients(
    client_filter: Option<&str>,
    hyphae_info: &Option<spore::ToolInfo>,
    rhizome_info: &Option<spore::ToolInfo>,
    verbose: u8,
) {
    let mut servers = Vec::new();
    if hyphae_info.is_some() {
        servers.push(ServerConfig {
            name: "hyphae".to_string(),
            command: "hyphae".to_string(),
            args: vec!["serve".to_string()],
        });
    }
    if rhizome_info.is_some() {
        servers.push(ServerConfig {
            name: "rhizome".to_string(),
            command: "rhizome".to_string(),
            args: vec!["serve".to_string(), "--expanded".to_string()],
        });
    }

    // Determine which clients to configure.
    let targets: Vec<McpClient> = if let Some(name) = client_filter {
        match McpClient::from_flag(name) {
            Some(c) => vec![c],
            None => {
                eprintln!(
                    "  {} Unknown client '{}'. Known: claude-code, codex-cli, cursor, windsurf, cline, continue, claude-desktop",
                    "!".yellow(),
                    name
                );
                return;
            }
        }
    } else {
        clients::detect_clients()
    };

    if targets.is_empty() {
        return;
    }

    println!();
    println!("{}", "Configuring detected MCP clients...".bold());

    let mut client_configured: Vec<String> = Vec::new();
    let needs_claude_init = targets
        .iter()
        .any(|client| matches!(client, McpClient::ClaudeCode));

    if servers.is_empty() && !needs_claude_init {
        return;
    }

    for target in &targets {
        if !servers.is_empty() {
            match clients::register_servers(*target, &servers, verbose) {
                Ok(true) => {
                    client_configured.push(target.name().to_string());
                }
                Ok(false) => {
                    eprintln!(
                        "  {} {} registration returned false",
                        "!".yellow(),
                        target.name()
                    );
                }
                Err(e) => {
                    eprintln!(
                        "  {} {} registration failed: {}",
                        "!".yellow(),
                        target.name(),
                        e
                    );
                }
            }
        }
    }

    if needs_claude_init {
        let patch_mode = super::PatchMode::Auto;
        if let Err(e) = super::run(true, false, false, patch_mode, verbose) {
            eprintln!("  {} Mycelium global init failed: {}", "!".yellow(), e);
        } else {
            client_configured.push("mycelium hooks + CLAUDE.md".to_string());
        }
    }

    if !client_configured.is_empty() {
        println!();
        println!("  {} MCP servers registered for:", "\u{2713}".green());
        for name in &client_configured {
            println!("    - {name}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_tool_status_installed_does_not_panic() {
        // Just verify no panics with various inputs
        print_tool_status("mycelium", Some("0.2.0"));
        print_tool_status("hyphae", Some("0.6.0"));
    }

    #[test]
    fn test_print_tool_status_missing_does_not_panic() {
        print_tool_status("cap", None);
        print_tool_status("rhizome", None);
    }

    #[test]
    fn test_discover_cap_does_not_panic() {
        // Cap likely not installed in test env — just verify no panic
        let _result = discover_cap();
    }

    #[test]
    fn test_detect_host_clients_does_not_panic() {
        let _result = clients::detect_host_clients();
    }
}
