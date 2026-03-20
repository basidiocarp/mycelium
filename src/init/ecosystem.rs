//! `mycelium init --ecosystem` — detect sibling tools and configure MCP clients.

use anyhow::{Context, Result};
use colored::Colorize;
use spore::{Tool, discover};
use std::process::Command;

use super::clients::{self, McpClient, ServerConfig};

/// Embedded session-summary hook script (POSIX sh, installed as Stop hook).
const SESSION_SUMMARY_HOOK: &str = include_str!("../../hooks/session-summary.sh");

/// Embedded Hyphae capture hooks (PostToolUse, installed to ~/.claude/hooks/basidiocarp/)
const CAPTURE_ERRORS_HOOK: &str = include_str!("../../hooks/capture-errors.js");
const CAPTURE_CORRECTIONS_HOOK: &str = include_str!("../../hooks/capture-corrections.js");
const CAPTURE_CODE_CHANGES_HOOK: &str = include_str!("../../hooks/capture-code-changes.js");
const HOOK_UTILS: &str = include_str!("../../hooks/lib/utils.js");

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

/// Check if an MCP server is already registered with Claude Code.
fn mcp_exists(name: &str) -> bool {
    Command::new("claude")
        .args(["mcp", "get", name])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Register an MCP server with Claude Code. Returns:
/// - `Ok(Some("registered"))` if newly registered
/// - `Ok(Some("already registered"))` if already present
/// - `Ok(None)` if registration failed
fn register_mcp(name: &str, args: &[&str], verbose: u8) -> Result<Option<&'static str>> {
    if mcp_exists(name) {
        if verbose > 0 {
            eprintln!("  {name} MCP already registered");
        }
        return Ok(Some("already registered"));
    }

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
    if output.status.success() {
        Ok(Some("registered"))
    } else {
        Ok(None)
    }
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

    // ── 3. Configure Claude Code (if available) ────────────────────────────
    if claude_is_available() {
        println!("{}", "Configuring Claude Code...".bold());
        println!();

        let mut configured = Vec::new();

        // Register hyphae MCP if installed
        if hyphae_info.is_some() {
            match register_mcp("hyphae", &["hyphae", "serve"], verbose) {
                Ok(Some(status)) => configured.push(if status == "already registered" {
                    "hyphae MCP (already registered)"
                } else {
                    "hyphae MCP"
                }),
                Ok(None) => eprintln!("  {} Failed to register hyphae MCP", "!".yellow()),
                Err(e) => eprintln!("  {} hyphae MCP registration error: {}", "!".yellow(), e),
            }
        }

        // Initialize hyphae database if it doesn't exist
        if let Some(data_dir) = hyphae_info
            .as_ref()
            .and(dirs::data_dir())
            .map(|d| d.join("hyphae"))
            .filter(|d| !d.join("hyphae.db").exists())
        {
            let _ = std::fs::create_dir_all(&data_dir);
            let _ = Command::new("hyphae").arg("stats").output();
            configured.push("hyphae database initialized");
        }

        // Register rhizome MCP if installed
        if rhizome_info.is_some() {
            match register_mcp("rhizome", &["rhizome", "serve", "--expanded"], verbose) {
                Ok(Some(status)) => configured.push(if status == "already registered" {
                    "rhizome MCP (already registered)"
                } else {
                    "rhizome MCP"
                }),
                Ok(None) => eprintln!("  {} Failed to register rhizome MCP", "!".yellow()),
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

        // Install session-summary Stop hook (captures session metrics in hyphae)
        match install_session_summary_hook(verbose) {
            Ok(true) => configured.push("session summary hook"),
            Ok(false) => { /* already present */ }
            Err(e) => eprintln!("  {} session summary hook: {}", "!".yellow(), e),
        }

        // Install capture hooks if hyphae is available
        if hyphae_info.is_some() {
            match install_capture_hooks(verbose) {
                Ok(count) if count > 0 => {
                    let msg = format!("{count} capture hooks");
                    configured.push(Box::leak(msg.into_boxed_str()));
                }
                Ok(_) => { /* already present */ }
                Err(e) => eprintln!("  {} capture hooks: {}", "!".yellow(), e),
            }
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

    // ── 3b. Configure additional MCP clients ────────────────────────────────
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

/// Install the session-summary Stop hook into `~/.claude/hooks/` and register it
/// in `~/.claude/settings.json`. Returns `Ok(true)` if newly installed,
/// `Ok(false)` if already present. Idempotent.
fn install_session_summary_hook(verbose: u8) -> Result<bool> {
    use super::hook::atomic_write;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    let claude_dir = super::claude_md::resolve_claude_dir()?;
    let hook_dir = claude_dir.join("hooks");
    fs::create_dir_all(&hook_dir)
        .with_context(|| format!("Failed to create hook dir: {}", hook_dir.display()))?;

    let hook_path = hook_dir.join("session-summary.sh");
    let hook_command = hook_path
        .to_str()
        .context("Hook path contains invalid UTF-8")?
        .to_string();

    // Write hook script (idempotent — skip if content matches)
    let needs_write = if hook_path.exists() {
        let existing = fs::read_to_string(&hook_path)?;
        existing != SESSION_SUMMARY_HOOK
    } else {
        true
    };

    if needs_write {
        fs::write(&hook_path, SESSION_SUMMARY_HOOK)
            .with_context(|| format!("Failed to write hook: {}", hook_path.display()))?;
        if verbose > 0 {
            eprintln!("  Wrote session-summary hook: {}", hook_path.display());
        }
    }

    // Set executable permissions
    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("Failed to chmod hook: {}", hook_path.display()))?;

    // Register Stop hook in settings.json (merge, don't overwrite)
    let settings_path = claude_dir.join("settings.json");
    let mut root: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .with_context(|| format!("Failed to read {}", settings_path.display()))?;
        if content.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {}", settings_path.display()))?
        }
    } else {
        serde_json::json!({})
    };

    // Check idempotency — look for session-summary.sh in Stop hooks
    if stop_hook_already_present(&root, &hook_command) {
        if verbose > 0 {
            eprintln!("  session-summary hook already in settings.json");
        }
        return Ok(false);
    }

    // Deep-merge: add Stop hook entry
    insert_stop_hook_entry(&mut root, &hook_command);

    // Atomic write settings.json
    let serialized =
        serde_json::to_string_pretty(&root).context("Failed to serialize settings.json")?;
    atomic_write(&settings_path, &serialized)?;

    if verbose > 0 {
        eprintln!("  Registered session-summary Stop hook in settings.json");
    }

    Ok(true)
}

/// Check if the session-summary Stop hook is already registered.
fn stop_hook_already_present(root: &serde_json::Value, hook_command: &str) -> bool {
    let stop_array = match root
        .get("hooks")
        .and_then(|h| h.get("Stop"))
        .and_then(|s| s.as_array())
    {
        Some(arr) => arr,
        None => return false,
    };

    stop_array
        .iter()
        .filter_map(|entry| entry.get("hooks")?.as_array())
        .flatten()
        .filter_map(|hook| hook.get("command")?.as_str())
        .any(|cmd| {
            cmd == hook_command
                || (cmd.contains("session-summary.sh")
                    && hook_command.contains("session-summary.sh"))
        })
}

/// Deep-merge a Stop hook entry into settings.json, preserving existing hooks.
fn insert_stop_hook_entry(root: &mut serde_json::Value, hook_command: &str) {
    let root_obj = match root.as_object_mut() {
        Some(obj) => obj,
        None => {
            *root = serde_json::json!({});
            root.as_object_mut()
                .expect("Just created object, must succeed")
        }
    };

    let hooks = root_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("hooks must be an object");

    let stop = hooks
        .entry("Stop")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .expect("Stop must be an array");

    stop.push(serde_json::json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": hook_command
        }]
    }));
}

/// Install Hyphae capture hooks to `~/.claude/hooks/basidiocarp/` and register them
/// in `~/.claude/settings.json`. Returns the number of hooks installed (0 if already present).
/// Idempotent.
fn install_capture_hooks(verbose: u8) -> Result<usize> {
    use super::hook::atomic_write;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    let claude_dir = super::claude_md::resolve_claude_dir()?;
    let hook_dir = claude_dir.join("hooks").join("basidiocarp");
    fs::create_dir_all(&hook_dir)
        .with_context(|| format!("Failed to create hook dir: {}", hook_dir.display()))?;

    let lib_dir = hook_dir.join("lib");
    fs::create_dir_all(&lib_dir)
        .with_context(|| format!("Failed to create lib dir: {}", lib_dir.display()))?;

    // ─────────────────────────────────────────────────────────────────────────
    // Install utils.js
    // ─────────────────────────────────────────────────────────────────────────
    let utils_path = lib_dir.join("utils.js");
    let utils_installed = if utils_path.exists() {
        let existing = fs::read_to_string(&utils_path)?;
        existing != HOOK_UTILS
    } else {
        true
    };

    if utils_installed {
        fs::write(&utils_path, HOOK_UTILS)
            .with_context(|| format!("Failed to write utils.js: {}", utils_path.display()))?;
        if verbose > 0 {
            eprintln!("  Wrote utils.js: {}", utils_path.display());
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Install capture hooks (with path adjustments)
    // ─────────────────────────────────────────────────────────────────────────
    let hooks = vec![
        ("capture-errors.js", CAPTURE_ERRORS_HOOK),
        ("capture-corrections.js", CAPTURE_CORRECTIONS_HOOK),
        ("capture-code-changes.js", CAPTURE_CODE_CHANGES_HOOK),
    ];

    let mut newly_installed = 0;

    for (name, content) in hooks {
        let hook_path = hook_dir.join(name);

        // Adjust require() paths in the hook content to use ./lib/utils
        let adjusted_content = content.replace("require('../lib/utils')", "require('./lib/utils')");

        let needs_write = if hook_path.exists() {
            let existing = fs::read_to_string(&hook_path)?;
            existing != adjusted_content
        } else {
            true
        };

        if needs_write {
            fs::write(&hook_path, &adjusted_content)
                .with_context(|| format!("Failed to write hook: {}", hook_path.display()))?;
            if verbose > 0 {
                eprintln!("  Wrote {}: {}", name, hook_path.display());
            }
            newly_installed += 1;
        }

        // Set executable permissions
        #[cfg(unix)]
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("Failed to chmod hook: {}", hook_path.display()))?;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Register PostToolUse hooks in settings.json
    // ─────────────────────────────────────────────────────────────────────────
    let settings_path = claude_dir.join("settings.json");
    let mut root: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .with_context(|| format!("Failed to read {}", settings_path.display()))?;
        if content.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {}", settings_path.display()))?
        }
    } else {
        serde_json::json!({})
    };

    // Check which hooks are already registered
    let capture_errors_path = hook_dir.join("capture-errors.js");
    let capture_corrections_path = hook_dir.join("capture-corrections.js");
    let capture_code_changes_path = hook_dir.join("capture-code-changes.js");

    let errors_present = post_tool_use_hook_already_present(&root, &capture_errors_path);
    let corrections_present = post_tool_use_hook_already_present(&root, &capture_corrections_path);
    let changes_present = post_tool_use_hook_already_present(&root, &capture_code_changes_path);

    // Register hooks that aren't already present
    let mut settings_changed = false;

    if !errors_present {
        insert_post_tool_use_hook_entry(
            &mut root,
            capture_errors_path
                .to_str()
                .context("Hook path contains invalid UTF-8")?,
            "Bash",
        );
        settings_changed = true;
    }

    if !corrections_present {
        insert_post_tool_use_hook_entry(
            &mut root,
            capture_corrections_path
                .to_str()
                .context("Hook path contains invalid UTF-8")?,
            "Write|Edit",
        );
        settings_changed = true;
    }

    if !changes_present {
        insert_post_tool_use_hook_entry(
            &mut root,
            capture_code_changes_path
                .to_str()
                .context("Hook path contains invalid UTF-8")?,
            "Write|Edit|Bash",
        );
        settings_changed = true;
    }

    // Write settings.json if anything changed
    if settings_changed {
        let serialized =
            serde_json::to_string_pretty(&root).context("Failed to serialize settings.json")?;
        atomic_write(&settings_path, &serialized)?;

        if verbose > 0 {
            eprintln!("  Registered capture hooks in settings.json");
        }
    } else if verbose > 0 {
        eprintln!("  Capture hooks already registered in settings.json");
    }

    Ok(newly_installed)
}

/// Check if a PostToolUse hook is already registered for the given command.
fn post_tool_use_hook_already_present(
    root: &serde_json::Value,
    hook_command_path: &std::path::Path,
) -> bool {
    let hook_command = match hook_command_path.to_str() {
        Some(s) => s,
        None => return false,
    };

    let post_tool_use_array = match root
        .get("hooks")
        .and_then(|h| h.get("PostToolUse"))
        .and_then(|s| s.as_array())
    {
        Some(arr) => arr,
        None => return false,
    };

    post_tool_use_array
        .iter()
        .filter_map(|entry| entry.get("hooks")?.as_array())
        .flatten()
        .filter_map(|hook| hook.get("command")?.as_str())
        .any(|cmd| {
            cmd == hook_command || (cmd.contains("capture-") && hook_command.contains("capture-"))
        })
}

/// Deep-merge a PostToolUse hook entry into settings.json, preserving existing hooks.
fn insert_post_tool_use_hook_entry(
    root: &mut serde_json::Value,
    hook_command: &str,
    matcher: &str,
) {
    let root_obj = match root.as_object_mut() {
        Some(obj) => obj,
        None => {
            *root = serde_json::json!({});
            root.as_object_mut()
                .expect("Just created object, must succeed")
        }
    };

    let hooks = root_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("hooks must be an object");

    let post_tool_use = hooks
        .entry("PostToolUse")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .expect("PostToolUse must be an array");

    post_tool_use.push(serde_json::json!({
        "matcher": matcher,
        "hooks": [{
            "type": "command",
            "command": hook_command
        }]
    }));
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

/// Configure detected MCP clients (other than Claude Code, which is handled separately).
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

    if servers.is_empty() {
        return;
    }

    // Determine which clients to configure
    let targets: Vec<McpClient> = if let Some(name) = client_filter {
        match McpClient::from_flag(name) {
            Some(c) => vec![c],
            None => {
                eprintln!(
                    "  {} Unknown client '{}'. Known: claude-code, cursor, windsurf, cline, continue, claude-desktop",
                    "!".yellow(),
                    name
                );
                return;
            }
        }
    } else {
        // No filter: detect all installed, skip Claude Code (already handled above)
        clients::detect_clients()
            .into_iter()
            .filter(|c| *c != McpClient::ClaudeCode)
            .collect()
    };

    if targets.is_empty() {
        return;
    }

    println!();
    println!("{}", "Configuring additional MCP clients...".bold());

    let mut client_configured = Vec::new();

    for target in &targets {
        if *target == McpClient::ClaudeCode && client_filter.is_none() {
            continue;
        }

        match clients::register_servers(*target, &servers, verbose) {
            Ok(true) => {
                client_configured.push(target.name());
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
    fn test_claude_is_available_does_not_panic() {
        let _result = claude_is_available();
    }

    #[test]
    fn test_session_summary_hook_embedded() {
        assert!(SESSION_SUMMARY_HOOK.starts_with("#!/bin/sh"));
        assert!(SESSION_SUMMARY_HOOK.contains("hyphae store"));
        assert!(SESSION_SUMMARY_HOOK.contains("session_id"));
    }

    #[test]
    fn test_stop_hook_already_present_exact() {
        let json = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "/Users/test/.claude/hooks/session-summary.sh"
                    }]
                }]
            }
        });
        assert!(stop_hook_already_present(
            &json,
            "/Users/test/.claude/hooks/session-summary.sh"
        ));
    }

    #[test]
    fn test_stop_hook_already_present_different_path() {
        let json = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "/home/user/.claude/hooks/session-summary.sh"
                    }]
                }]
            }
        });
        assert!(stop_hook_already_present(
            &json,
            "/Users/other/.claude/hooks/session-summary.sh"
        ));
    }

    #[test]
    fn test_stop_hook_not_present_empty() {
        let json = serde_json::json!({});
        assert!(!stop_hook_already_present(
            &json,
            "/Users/test/.claude/hooks/session-summary.sh"
        ));
    }

    #[test]
    fn test_stop_hook_not_present_other_hooks() {
        let json = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/Users/test/.claude/hooks/mycelium-rewrite.sh"
                    }]
                }]
            }
        });
        assert!(!stop_hook_already_present(
            &json,
            "/Users/test/.claude/hooks/session-summary.sh"
        ));
    }

    #[test]
    fn test_insert_stop_hook_entry_empty() {
        let mut json = serde_json::json!({});
        insert_stop_hook_entry(&mut json, "/test/session-summary.sh");

        let stop = json["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert_eq!(
            stop[0]["hooks"][0]["command"].as_str().unwrap(),
            "/test/session-summary.sh"
        );
    }

    #[test]
    fn test_insert_stop_hook_preserves_existing() {
        let mut json = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/test/mycelium-rewrite.sh"
                    }]
                }]
            }
        });
        insert_stop_hook_entry(&mut json, "/test/session-summary.sh");

        // PreToolUse preserved
        assert!(json["hooks"]["PreToolUse"].is_array());
        // Stop added
        let stop = json["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
    }

    #[test]
    fn test_capture_errors_hook_embedded() {
        assert!(CAPTURE_ERRORS_HOOK.starts_with("#!/usr/bin/env node"));
        assert!(CAPTURE_ERRORS_HOOK.contains("require('../lib/utils')"));
    }

    #[test]
    fn test_capture_corrections_hook_embedded() {
        assert!(CAPTURE_CORRECTIONS_HOOK.starts_with("#!/usr/bin/env node"));
        assert!(CAPTURE_CORRECTIONS_HOOK.contains("require('../lib/utils')"));
    }

    #[test]
    fn test_capture_code_changes_hook_embedded() {
        assert!(CAPTURE_CODE_CHANGES_HOOK.starts_with("#!/usr/bin/env node"));
        assert!(CAPTURE_CODE_CHANGES_HOOK.contains("require('../lib/utils')"));
    }

    #[test]
    fn test_hook_utils_embedded() {
        assert!(HOOK_UTILS.contains("module.exports"));
        assert!(HOOK_UTILS.contains("function commandExists"));
        assert!(HOOK_UTILS.contains("function log"));
    }

    #[test]
    fn test_post_tool_use_hook_already_present_exact() {
        let json = serde_json::json!({
            "hooks": {
                "PostToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/Users/test/.claude/hooks/basidiocarp/capture-errors.js"
                    }]
                }]
            }
        });

        let path = std::path::Path::new("/Users/test/.claude/hooks/basidiocarp/capture-errors.js");
        assert!(post_tool_use_hook_already_present(&json, path));
    }

    #[test]
    fn test_post_tool_use_hook_not_present_empty() {
        let json = serde_json::json!({});
        let path = std::path::Path::new("/Users/test/.claude/hooks/basidiocarp/capture-errors.js");
        assert!(!post_tool_use_hook_already_present(&json, path));
    }

    #[test]
    fn test_insert_post_tool_use_hook_entry_empty() {
        let mut json = serde_json::json!({});
        insert_post_tool_use_hook_entry(&mut json, "/test/capture-errors.js", "Bash");

        let post_tool_use = json["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(post_tool_use.len(), 1);
        assert_eq!(post_tool_use[0]["matcher"].as_str().unwrap(), "Bash");
        assert_eq!(
            post_tool_use[0]["hooks"][0]["command"].as_str().unwrap(),
            "/test/capture-errors.js"
        );
    }

    #[test]
    fn test_insert_post_tool_use_hook_preserves_stop_hooks() {
        let mut json = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "/test/session-summary.sh"
                    }]
                }]
            }
        });

        insert_post_tool_use_hook_entry(&mut json, "/test/capture-errors.js", "Bash");

        // Stop preserved
        assert!(json["hooks"]["Stop"].is_array());
        assert_eq!(json["hooks"]["Stop"].as_array().unwrap().len(), 1);
        // PostToolUse added
        assert!(json["hooks"]["PostToolUse"].is_array());
        assert_eq!(json["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
    }
}
