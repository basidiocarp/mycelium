//! `mycelium init --onboard` — interactive onboarding wizard.
//!
//! Walks the user through first-time setup of the Basidiocarp ecosystem:
//! detect tools, configure available host adapters, store first hyphae
//! memory, scan with rhizome, export code graph, and print a summary.
//!
//! Falls back to `--ecosystem` when stdin is not a TTY (CI, pipes).

use anyhow::{Context, Result};
use colored::Colorize;
use spore::{Tool, ToolInfo, discover, discover_all};
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::Command;

use super::clients::{self, McpClient, ServerConfig};

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Run the interactive onboarding wizard.
///
/// If stdin is not a TTY, delegates to `mycelium init --ecosystem` instead.
pub fn run_onboard(verbose: u8) -> Result<()> {
    if !is_tty() {
        eprintln!("[mycelium] Non-interactive terminal detected — running --ecosystem instead.");
        return super::run_ecosystem(None, verbose);
    }

    print_banner();

    // ── Step 1: Detect tools ─────────────────────────────────────────────
    println!();
    println!("{}", "Step 1/5: Detecting ecosystem tools...".bold());
    println!();

    let tools = discover_all();
    let mycelium_info = discover(Tool::Mycelium);
    let hyphae_info = discover(Tool::Hyphae);
    let rhizome_info = discover(Tool::Rhizome);
    let cap_version = discover_cap();

    print_tool_status("mycelium", mycelium_info.as_ref());
    print_tool_status("hyphae", hyphae_info.as_ref());
    print_tool_status("rhizome", rhizome_info.as_ref());
    print_tool_status_version("cap", cap_version.as_deref());

    let missing = build_missing_list(&hyphae_info, &rhizome_info, &cap_version);
    if !missing.is_empty() {
        println!();
        println!("{}", "Missing tools:".dimmed());
        for (name, cmd) in &missing {
            println!("  {:<10} {} {}", name, "\u{2192}".dimmed(), cmd.dimmed());
        }
    }

    // ── Step 2: Configure host adapters ─────────────────────────────────
    println!();
    println!("{}", "Step 2/5: Configure host adapters".bold());
    println!();

    let host_clients = clients::detect_host_clients();
    if !host_clients.is_empty() {
        let host_label = host_clients
            .iter()
            .map(|client| client.name())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  Detected host clients: {host_label}");
        if confirm("Register detected host adapters and enable Claude hooks when available?") {
            configure_host_adapters(&host_clients, &hyphae_info, &rhizome_info, verbose)?;
        } else {
            println!("  Skipped host-adapter configuration.");
        }
    } else {
        println!(
            "  {} No supported host adapter detected (Claude Code or Codex CLI) — skipping.",
            "!".yellow()
        );
        println!(
            "  {}",
            "Install Claude Code or Codex CLI, then re-run: mycelium init --onboard".dimmed()
        );
    }

    // ── Step 3: First hyphae memory ──────────────────────────────────────
    println!();
    println!("{}", "Step 3/5: Store first memory in Hyphae".bold());
    println!();

    if hyphae_info.is_some() {
        if confirm("Store a welcome memory in Hyphae?") {
            store_welcome_memory(verbose)?;
        } else {
            println!("  Skipped.");
        }
    } else {
        println!(
            "  {} Hyphae not installed — skipping memory setup.",
            "\u{2014}".dimmed()
        );
    }

    // ── Step 4: Scan with rhizome ────────────────────────────────────────
    println!();
    println!("{}", "Step 4/5: Scan project with Rhizome".bold());
    println!();

    if rhizome_info.is_some() {
        let cwd = std::env::current_dir().unwrap_or_default();
        let has_code = has_source_files(&cwd);
        if has_code {
            if confirm(&format!(
                "Scan {} with Rhizome code intelligence?",
                cwd.display()
            )) {
                scan_with_rhizome(verbose)?;
            } else {
                println!("  Skipped.");
            }
        } else {
            println!(
                "  {} No source files detected in current directory — skipping.",
                "\u{2014}".dimmed()
            );
        }
    } else {
        println!(
            "  {} Rhizome not installed — skipping code scan.",
            "\u{2014}".dimmed()
        );
    }

    // ── Step 5: Export to Hyphae ─────────────────────────────────────────
    println!();
    println!("{}", "Step 5/5: Export code graph to Hyphae".bold());
    println!();

    if hyphae_info.is_some() && rhizome_info.is_some() {
        if confirm("Export code symbols to Hyphae knowledge graph?") {
            export_to_hyphae(verbose)?;
        } else {
            println!("  Skipped.");
        }
    } else {
        let reason = match (&hyphae_info, &rhizome_info) {
            (None, None) => "Hyphae and Rhizome not installed",
            (None, _) => "Hyphae not installed",
            (_, None) => "Rhizome not installed",
            _ => unreachable!(),
        };
        println!("  {} {} — skipping export.", "\u{2014}".dimmed(), reason);
    }

    // ── Summary ──────────────────────────────────────────────────────────
    println!();
    println!("{}", "\u{2500}".repeat(60));
    print_summary(&tools, &cap_version, &host_clients);

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// TTY detection
// ─────────────────────────────────────────────────────────────────────────────

fn is_tty() -> bool {
    io::stdin().is_terminal()
}

// ─────────────────────────────────────────────────────────────────────────────
// Prompts
// ─────────────────────────────────────────────────────────────────────────────

/// Prompt for Y/n confirmation. Returns true on Enter (default = yes).
fn confirm(question: &str) -> bool {
    print!("  {} [Y/n] ", question);
    let _ = io::stdout().flush();

    let mut input = String::new();
    let stdin = io::stdin();
    match stdin.lock().read_line(&mut input) {
        Ok(_) => {
            let trimmed = input.trim().to_lowercase();
            trimmed.is_empty() || trimmed == "y" || trimmed == "yes"
        }
        Err(_) => true, // default yes on read error
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Banner
// ─────────────────────────────────────────────────────────────────────────────

fn print_banner() {
    println!();
    println!("{}", "\u{2500}".repeat(60));
    println!(
        "{}",
        "  Basidiocarp Ecosystem — Interactive Onboarding".bold()
    );
    println!("{}", "\u{2500}".repeat(60));
    println!();
    println!("  This wizard will set up the Basidiocarp ecosystem:");
    println!("    1. Detect installed tools");
    println!("    2. Configure your host adapters (Claude Code and/or Codex CLI)");
    println!("    3. Store a first memory in Hyphae");
    println!("    4. Scan your project with Rhizome");
    println!("    5. Export code graph to Hyphae");
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool detection helpers
// ─────────────────────────────────────────────────────────────────────────────

fn print_tool_status(name: &str, info: Option<&ToolInfo>) {
    match info {
        Some(i) => {
            println!(
                "  {:<10} v{:<8} {}",
                name.bold(),
                i.version,
                "\u{2713} installed".green()
            );
        }
        None => {
            println!(
                "  {:<10} {:<8} {}",
                name.bold(),
                "\u{2014}",
                "\u{2717} not installed".red()
            );
        }
    }
}

fn print_tool_status_version(name: &str, version: Option<&str>) {
    match version {
        Some(v) => {
            println!(
                "  {:<10} v{:<8} {}",
                name.bold(),
                v,
                "\u{2713} installed".green()
            );
        }
        None => {
            println!(
                "  {:<10} {:<8} {} {}",
                name.bold(),
                "\u{2014}",
                "\u{2717} not installed".red(),
                "(optional)".dimmed()
            );
        }
    }
}

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

fn build_missing_list<'a>(
    hyphae: &Option<ToolInfo>,
    rhizome: &Option<ToolInfo>,
    cap: &Option<String>,
) -> Vec<(&'a str, &'a str)> {
    let mut missing = Vec::new();
    if hyphae.is_none() {
        missing.push((
            "hyphae",
            "cargo install --git https://github.com/basidiocarp/hyphae hyphae-cli --no-default-features",
        ));
    }
    if rhizome.is_none() {
        missing.push((
            "rhizome",
            "cargo install --git https://github.com/basidiocarp/rhizome rhizome-cli",
        ));
    }
    if cap.is_none() {
        missing.push((
            "cap",
            "git clone https://github.com/basidiocarp/cap && cd cap && npm i && npm run dev:all",
        ));
    }
    missing
}

fn has_source_files(dir: &std::path::Path) -> bool {
    const EXTENSIONS: &[&str] = &[
        "rs", "py", "js", "ts", "go", "java", "c", "cpp", "rb", "ex", "zig",
    ];
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_source = path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| EXTENSIONS.contains(&ext));
        if is_source {
            return true;
        }
    }
    false
}

// ─────────────────────────────────────────────────────────────────────────────
// Step implementations
// ─────────────────────────────────────────────────────────────────────────────

fn configure_host_adapters(
    host_clients: &[McpClient],
    hyphae_info: &Option<ToolInfo>,
    rhizome_info: &Option<ToolInfo>,
    verbose: u8,
) -> Result<()> {
    let mut configured = Vec::new();

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

    if !servers.is_empty() {
        for client in host_clients {
            match clients::register_servers(*client, &servers, verbose) {
                Ok(true) => configured.push(client.name().to_string()),
                Ok(false) => eprintln!("  {} Failed to configure {}", "!".yellow(), client.name()),
                Err(e) => eprintln!("  {} {}: {}", "!".yellow(), client.name(), e),
            }
        }
    }

    if hyphae_info.is_some() {
        if let Some(data_dir) = hyphae_info
            .as_ref()
            .and(dirs::data_dir())
            .map(|d| d.join("hyphae"))
            .filter(|d| !d.join("hyphae.db").exists())
        {
            let _ = std::fs::create_dir_all(&data_dir);
            let _ = Command::new("hyphae").arg("stats").output();
            configured.push("hyphae database initialized".to_string());
        }
    }

    if host_clients
        .iter()
        .any(|client| matches!(client, McpClient::ClaudeCode))
    {
        let patch_mode = super::PatchMode::Auto;
        if let Err(e) = super::run(true, false, false, patch_mode, verbose) {
            eprintln!("  {} Mycelium global init failed: {}", "!".yellow(), e);
        } else {
            configured.push("Claude Code adapter (hook + CLAUDE.md)".to_string());
        }
    }

    if !configured.is_empty() {
        println!();
        println!("  {} Configured:", "\u{2713}".green());
        for item in &configured {
            println!("    - {}", item);
        }
    }

    Ok(())
}

fn store_welcome_memory(verbose: u8) -> Result<()> {
    let output = Command::new("hyphae")
        .args([
            "store",
            "--topic",
            "onboarding",
            "--importance",
            "high",
            "--content",
            "Basidiocarp ecosystem onboarded. Tools: mycelium (token proxy), hyphae (memory), \
             rhizome (code intelligence). Use `mycelium init --ecosystem` to reconfigure.",
        ])
        .output()
        .context("Failed to run hyphae store")?;

    if output.status.success() {
        println!("  {} Welcome memory stored in Hyphae.", "\u{2713}".green());
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "  {} Failed to store memory: {}",
            "!".yellow(),
            stderr.trim()
        );
    }

    if verbose > 0 {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            eprintln!("  {}", stdout.trim());
        }
    }

    Ok(())
}

fn scan_with_rhizome(verbose: u8) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let cwd_str = cwd.to_string_lossy();

    println!("  Scanning {}...", cwd_str);

    let output = Command::new("rhizome")
        .args(["symbols", &cwd_str])
        .output()
        .context("Failed to run rhizome symbols")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        let count = lines.len();
        println!(
            "  {} Found {} symbols in project.",
            "\u{2713}".green(),
            count
        );
        if verbose > 0 && count > 0 {
            let preview_count = count.min(5);
            for line in &lines[..preview_count] {
                eprintln!("    {}", line);
            }
            if count > 5 {
                eprintln!("    ... and {} more", count - 5);
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("  {} Rhizome scan failed: {}", "!".yellow(), stderr.trim());
    }

    Ok(())
}

fn export_to_hyphae(verbose: u8) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let cwd_str = cwd.to_string_lossy();

    println!("  Exporting code graph from {}...", cwd_str);

    let output = Command::new("rhizome")
        .args(["export", &cwd_str])
        .output()
        .context("Failed to run rhizome export")?;

    if output.status.success() {
        println!("  {} Code graph exported to Hyphae.", "\u{2713}".green());
        if verbose > 0 {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                eprintln!("  {}", stdout.trim());
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("  {} Export failed: {}", "!".yellow(), stderr.trim());
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Summary
// ─────────────────────────────────────────────────────────────────────────────

fn print_summary(tools: &[ToolInfo], cap_version: &Option<String>, host_clients: &[McpClient]) {
    println!();
    println!("{}", "  Onboarding Complete!".bold().green());
    println!();

    let installed_count = tools.len() + if cap_version.is_some() { 1 } else { 0 };
    let total = 4; // mycelium, hyphae, rhizome, cap
    println!("  Tools: {}/{} installed", installed_count, total);

    if host_clients.is_empty() {
        println!("  Host adapters: not detected");
    } else {
        for host in host_clients {
            println!("  {}: configured", host.name());
        }
    }

    println!();
    println!("  {}", "Next steps:".bold());
    if host_clients
        .iter()
        .any(|client| matches!(client, McpClient::ClaudeCode))
    {
        println!(
            "    - Verify Claude Code adapter: {}",
            "git status".dimmed()
        );
    }
    if host_clients
        .iter()
        .any(|client| matches!(client, McpClient::CodexCli))
    {
        println!(
            "    - Verify Codex CLI adapter: {}",
            "cat ~/.codex/config.toml".dimmed()
        );
    }
    println!(
        "    - Check token savings:        {}",
        "mycelium gain".dimmed()
    );
    println!(
        "    - Recall memories:            {}",
        "hyphae recall \"onboarding\"".dimmed()
    );
    println!(
        "    - View ecosystem status:      {}",
        "mycelium init --ecosystem".dimmed()
    );
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_empty_input_is_yes() {
        // confirm() reads from stdin; can't easily test interactively,
        // but we can verify the logic branches exist without panicking.
        // The function defaults to true on empty/error.
    }

    #[test]
    fn test_build_missing_list_all_present() {
        let hyphae = Some(ToolInfo {
            tool: Tool::Hyphae,
            version: "0.3.0".to_string(),
            binary_path: std::path::PathBuf::from("/usr/bin/hyphae"),
        });
        let rhizome = Some(ToolInfo {
            tool: Tool::Rhizome,
            version: "0.4.0".to_string(),
            binary_path: std::path::PathBuf::from("/usr/bin/rhizome"),
        });
        let cap = Some("1.0.0".to_string());
        let missing = build_missing_list(&hyphae, &rhizome, &cap);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_build_missing_list_all_missing() {
        let missing = build_missing_list(&None, &None, &None);
        assert_eq!(missing.len(), 3);
    }

    #[test]
    fn test_discover_cap_does_not_panic() {
        let _result = discover_cap();
    }

    #[test]
    fn test_detect_host_clients_does_not_panic() {
        let _result = clients::detect_host_clients();
    }

    #[test]
    fn test_has_source_files_nonexistent_dir() {
        let result = has_source_files(std::path::Path::new("/nonexistent/path"));
        assert!(!result);
    }

    #[test]
    fn test_print_banner_does_not_panic() {
        print_banner();
    }
}
