//! `mycelium init --ecosystem` — detect sibling tools and configure Claude Code.

use anyhow::Result;
use colored::Colorize;
use spore::{Tool, discover};
use std::process::Command;

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

/// Check if `claude` binary is in PATH.
fn claude_is_available() -> bool {
    Command::new("claude")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Register an MCP server with Claude Code.
fn register_mcp(name: &str, args: &[&str], verbose: u8) -> Result<bool> {
    let mut cmd = Command::new("claude");
    cmd.arg("mcp")
        .arg("add")
        .arg("--scope")
        .arg("user")
        .arg(name);
    cmd.arg("--");
    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!(
            "  Running: claude mcp add --scope user {} -- {}",
            name,
            args.join(" ")
        );
    }

    let output = cmd.output()?;
    Ok(output.status.success())
}

/// Main entry point for `mycelium init --ecosystem`.
pub fn run_ecosystem(verbose: u8) -> Result<()> {
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

    // ── 3. Configure Claude Code (if available) ────────────────────────────
    if claude_is_available() {
        println!("{}", "Configuring Claude Code...".bold());
        println!();

        let mut configured = Vec::new();

        // Register hyphae MCP if installed
        if hyphae_info.is_some() {
            match register_mcp("hyphae", &["hyphae", "serve"], verbose) {
                Ok(true) => configured.push("hyphae MCP"),
                Ok(false) => eprintln!("  {} Failed to register hyphae MCP", "!".yellow()),
                Err(e) => eprintln!("  {} hyphae MCP registration error: {}", "!".yellow(), e),
            }
        }

        // Register rhizome MCP if installed
        if rhizome_info.is_some() {
            match register_mcp("rhizome", &["rhizome", "serve", "--expanded"], verbose) {
                Ok(true) => configured.push("rhizome MCP"),
                Ok(false) => eprintln!("  {} Failed to register rhizome MCP", "!".yellow()),
                Err(e) => eprintln!("  {} rhizome MCP registration error: {}", "!".yellow(), e),
            }
        }

        // Run the existing init --global logic for mycelium instructions
        let patch_mode = super::PatchMode::Auto;
        if let Err(e) = super::run(true, false, false, patch_mode, verbose) {
            eprintln!("  {} Mycelium global init failed: {}", "!".yellow(), e);
        } else {
            configured.push("mycelium hooks + CLAUDE.md");
        }

        if !configured.is_empty() {
            println!();
            println!("  {} Configured:", "\u{2713}".green());
            for item in &configured {
                println!("    - {}", item);
            }
        }
    } else {
        println!(
            "  {} {} not found in PATH — skipping Claude Code configuration.",
            "!".yellow(),
            "claude".bold()
        );
        println!("    Install Claude Code first, then re-run: mycelium init --ecosystem");
    }

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
        missing.push(("cap", "npm install -g @basidiocarp/cap"));
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
                "cap" => " (optional: npm install -g @basidiocarp/cap)",
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
    fn test_claude_is_available_does_not_panic() {
        let _result = claude_is_available();
    }
}
