use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use serde::Deserialize;
use serde_json::{Value, json};
use spore::editors::{self, Editor};
use spore::{Tool, discover, jsonrpc::Request};
use toml::value::Table;

static ONBOARD_CANCELLED: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
static INSTALL_SIGINT_HANDLER: Once = Once::new();

#[cfg(unix)]
extern "C" fn handle_sigint(_: i32) {
    ONBOARD_CANCELLED.store(true, Ordering::SeqCst);
}

#[cfg(unix)]
fn install_sigint_handler() {
    INSTALL_SIGINT_HANDLER.call_once(|| unsafe {
        libc::signal(libc::SIGINT, handle_sigint as *const () as usize);
    });
}

#[cfg(not(unix))]
fn install_sigint_handler() {}

#[derive(Debug, Clone)]
struct ToolStatus {
    name: &'static str,
    installed: bool,
    detail: String,
}

#[derive(Debug, Default)]
struct OnboardingSummary {
    claude_setup: String,
    codex_setup: String,
    memory_status: String,
    rhizome_scan_status: String,
    hyphae_quick_start: Option<String>,
    rhizome_quick_start: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HyphaeOnboardInfo {
    total_memories: usize,
    total_memoirs: usize,
    quick_start: String,
}

#[derive(Debug, Deserialize)]
struct RhizomeOnboardInfo {
    backend: String,
    project_root: String,
    quick_start: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CodexServerUpdate {
    added: Vec<String>,
    already_present: Vec<String>,
}

#[derive(Debug)]
enum PromptDecision {
    Yes,
    No,
    Cancelled,
}

#[derive(Debug)]
enum PromptText {
    Value(String),
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostSetupPlan {
    init_command: &'static str,
    scope_label: &'static str,
}

pub(super) fn run(global: bool, verbose: u8) -> Result<()> {
    ONBOARD_CANCELLED.store(false, Ordering::SeqCst);
    install_sigint_handler();
    let setup_plan = host_setup_plan(global);

    let current_dir = std::env::current_dir().context("Failed to determine current directory")?;
    let hyphae = detect_binary_tool(Tool::Hyphae, "Hyphae");
    let rhizome = detect_binary_tool(Tool::Rhizome, "Rhizome");
    let cap = detect_cap(&current_dir);

    let mut summary = OnboardingSummary {
        claude_setup: "skipped".to_string(),
        codex_setup: "skipped".to_string(),
        memory_status: "skipped".to_string(),
        rhizome_scan_status: "skipped".to_string(),
        hyphae_quick_start: None,
        rhizome_quick_start: None,
    };

    println!(
        "{}",
        "Interactive Ecosystem Onboarding".bold().bright_cyan()
    );
    println!(
        "{}",
        "Configure the Basidiocarp toolchain step by step. Press Ctrl+C at any prompt to exit."
            .dimmed()
    );
    println!();

    print_step_heading(1, "Detect installed tools");
    print_tool_status(&hyphae);
    print_tool_status(&rhizome);
    print_tool_status(&cap);
    println!();

    print_step_heading(2, "Configure host integration and MCP servers");
    match prompt_yes_no(
        &format!(
            "Apply the recommended {} setup now?",
            setup_plan.scope_label
        ),
        true,
    )? {
        PromptDecision::Yes => {
            summary.claude_setup =
                match super::run_default_mode(global, super::PatchMode::Auto, verbose) {
                    Ok(()) => format!(
                        "configured {} Claude integration",
                        setup_plan.scope_label
                    ),
                    Err(error) => format!("warning: Claude setup failed: {error}"),
                };

            summary.codex_setup = if hyphae.installed || rhizome.installed {
                match configure_codex_mcp_servers(&hyphae, &rhizome) {
                    Ok(update) if update.added.is_empty() && !update.already_present.is_empty() => {
                        format!("already configured ({})", update.already_present.join(", "))
                    }
                    Ok(update) if !update.added.is_empty() => {
                        format!("configured ({})", update.added.join(", "))
                    }
                    Ok(_) => "skipped: no supported servers detected".to_string(),
                    Err(error) => format!("warning: Codex MCP setup failed: {error}"),
                }
            } else {
                "skipped: Hyphae/Rhizome binaries not detected".to_string()
            };
        }
        PromptDecision::No => {
            println!(
                "Skipped setup. You can run `{}` later.",
                setup_plan.init_command.bold()
            );
            summary.claude_setup = "skipped by user".to_string();
            summary.codex_setup = "skipped by user".to_string();
        }
        PromptDecision::Cancelled => return finish_cancelled(),
    }
    println!();

    if hyphae.installed {
        summary.hyphae_quick_start = fetch_hyphae_onboard()
            .map(|info| {
                println!(
                    "{} {} memories, {} memoirs",
                    "Hyphae overview:".green().bold(),
                    info.total_memories,
                    info.total_memoirs
                );
                info.quick_start
            })
            .ok();
    }

    print_step_heading(3, "Store your first memory");
    if !hyphae.installed {
        println!("Skipped: Hyphae is not available on PATH.");
        summary.memory_status = "skipped: Hyphae not available".to_string();
    } else {
        match prompt_text("What are you working on today? Leave blank to skip.")? {
            PromptText::Value(content) => match store_first_memory(&content) {
                Ok(()) => {
                    println!("{}", "Stored your first Hyphae memory.".green());
                    summary.memory_status = "stored".to_string();
                }
                Err(error) => {
                    println!("{} {}", "Warning:".yellow().bold(), error);
                    summary.memory_status = format!("warning: {error}");
                }
            },
            PromptText::Skipped => {
                println!("Skipped first memory.");
                summary.memory_status = "skipped".to_string();
            }
            PromptText::Cancelled => return finish_cancelled(),
        }
    }
    println!();

    if rhizome.installed {
        summary.rhizome_quick_start = fetch_rhizome_onboard()
            .map(|info| {
                println!("{} {}", "Rhizome backend:".green().bold(), info.backend);
                println!(
                    "{} {}",
                    "Rhizome project root:".green().bold(),
                    info.project_root
                );
                info.quick_start
            })
            .ok();
    }

    print_step_heading(4, "Scan the current directory with Rhizome");
    if !rhizome.installed {
        println!("Skipped: Rhizome is not available on PATH.");
        summary.rhizome_scan_status = "skipped: Rhizome not available".to_string();
    } else {
        match prompt_yes_no(
            &format!("Run `rhizome summarize {}` now?", current_dir.display()),
            true,
        )? {
            PromptDecision::Yes => match run_rhizome_summary(&current_dir) {
                Ok(summary_text) => {
                    println!("{}", excerpt_lines(&summary_text, 12));
                    summary.rhizome_scan_status =
                        format!("completed for {}", current_dir.display());
                }
                Err(error) => {
                    println!("{} {}", "Warning:".yellow().bold(), error);
                    summary.rhizome_scan_status = format!("warning: {error}");
                }
            },
            PromptDecision::No => {
                println!("Skipped Rhizome project summary.");
                summary.rhizome_scan_status = "skipped".to_string();
            }
            PromptDecision::Cancelled => return finish_cancelled(),
        }
    }
    println!();

    print_step_heading(5, "Summary");
    print_summary_line("Claude setup", &summary.claude_setup);
    print_summary_line("Codex MCP servers", &summary.codex_setup);
    print_summary_line("First memory", &summary.memory_status);
    print_summary_line("Rhizome scan", &summary.rhizome_scan_status);
    print_summary_line(
        "Cap dashboard",
        if cap.installed {
            &cap.detail
        } else {
            "not detected"
        },
    );
    if let Some(quick_start) = &summary.hyphae_quick_start {
        print_summary_line("Hyphae quick start", quick_start);
    }
    if let Some(quick_start) = &summary.rhizome_quick_start {
        print_summary_line("Rhizome quick start", quick_start);
    }

    Ok(())
}

fn host_setup_plan(global: bool) -> HostSetupPlan {
    if global {
        HostSetupPlan {
            init_command: "mycelium init -g",
            scope_label: "global",
        }
    } else {
        HostSetupPlan {
            init_command: "mycelium init",
            scope_label: "project-local",
        }
    }
}

fn detect_binary_tool(tool: Tool, name: &'static str) -> ToolStatus {
    match discover(tool) {
        Some(info) => ToolStatus {
            name,
            installed: true,
            detail: info.binary_path.display().to_string(),
        },
        None => ToolStatus {
            name,
            installed: false,
            detail: "not found on PATH".to_string(),
        },
    }
}

fn detect_cap(current_dir: &Path) -> ToolStatus {
    if let Some(path) = find_workspace_cap(current_dir) {
        return ToolStatus {
            name: "Cap",
            installed: true,
            detail: format!("workspace repo at {}", path.display()),
        };
    }

    if let Some(path) = crate::platform::command_path("cap") {
        return ToolStatus {
            name: "Cap",
            installed: true,
            detail: path.display().to_string(),
        };
    }

    ToolStatus {
        name: "Cap",
        installed: false,
        detail: "dashboard repo not detected nearby".to_string(),
    }
}

fn find_workspace_cap(current_dir: &Path) -> Option<PathBuf> {
    current_dir.ancestors().find_map(|ancestor| {
        let cap_dir = ancestor.join("cap");
        if cap_dir.join("package.json").exists() {
            Some(cap_dir)
        } else {
            None
        }
    })
}

fn configure_codex_mcp_servers(
    hyphae: &ToolStatus,
    rhizome: &ToolStatus,
) -> Result<CodexServerUpdate> {
    let path =
        editors::config_path(Editor::CodexCli).context("Codex CLI config path is not available")?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let existing = if path.exists() {
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?
    } else {
        String::new()
    };

    let (updated, result) = upsert_codex_servers(&existing, hyphae, rhizome)?;
    if updated != existing {
        fs::write(&path, updated).with_context(|| format!("Failed to write {}", path.display()))?;
    }

    Ok(result)
}

fn upsert_codex_servers(
    existing: &str,
    hyphae: &ToolStatus,
    rhizome: &ToolStatus,
) -> Result<(String, CodexServerUpdate)> {
    let mut root = if existing.trim().is_empty() {
        toml::Value::Table(Table::new())
    } else {
        toml::from_str(existing).context("Failed to parse existing Codex config as TOML")?
    };

    let root_table = root
        .as_table_mut()
        .ok_or_else(|| anyhow!("Codex config root must be a TOML table"))?;

    let mcp_servers = root_table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(Table::new()));
    let mcp_table = mcp_servers
        .as_table_mut()
        .ok_or_else(|| anyhow!("`mcp_servers` must be a TOML table"))?;

    let mut result = CodexServerUpdate::default();

    if hyphae.installed {
        upsert_server(
            mcp_table,
            "hyphae",
            hyphae.detail.as_str(),
            &["serve"],
            &mut result,
        );
    }

    if rhizome.installed {
        upsert_server(
            mcp_table,
            "rhizome",
            rhizome.detail.as_str(),
            &["serve", "--expanded"],
            &mut result,
        );
    }

    let serialized = toml::to_string_pretty(&root).context("Failed to serialize Codex config")?;
    Ok((serialized, result))
}

fn upsert_server(
    mcp_table: &mut Table,
    name: &str,
    command: &str,
    args: &[&str],
    result: &mut CodexServerUpdate,
) {
    let desired_args = args
        .iter()
        .map(|arg| toml::Value::String((*arg).to_string()))
        .collect::<Vec<_>>();

    let already_present = mcp_table
        .get(name)
        .and_then(toml::Value::as_table)
        .is_some_and(|server| {
            server
                .get("command")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| value == command)
                && server
                    .get("args")
                    .and_then(toml::Value::as_array)
                    .is_some_and(|value| value == &desired_args)
        });

    if already_present {
        result.already_present.push(name.to_string());
        return;
    }

    let mut server = Table::new();
    server.insert(
        "command".to_string(),
        toml::Value::String(command.to_string()),
    );
    server.insert("args".to_string(), toml::Value::Array(desired_args));
    mcp_table.insert(name.to_string(), toml::Value::Table(server));
    result.added.push(name.to_string());
}

fn fetch_hyphae_onboard() -> Result<HyphaeOnboardInfo> {
    let text = call_tool_text(
        Tool::Hyphae,
        &["serve"],
        "hyphae_onboard",
        json!({}),
        Duration::from_secs(10),
    )?;
    serde_json::from_str(&text).context("Failed to parse hyphae_onboard response")
}

fn store_first_memory(content: &str) -> Result<()> {
    let _ = call_tool_text(
        Tool::Hyphae,
        &["serve"],
        "hyphae_memory_store",
        json!({
            "topic": "project",
            "content": content,
            "importance": "high",
            "keywords": ["onboarding", "first-memory"],
        }),
        Duration::from_secs(10),
    )?;
    Ok(())
}

fn fetch_rhizome_onboard() -> Result<RhizomeOnboardInfo> {
    let text = call_tool_text(
        Tool::Rhizome,
        &["serve", "--expanded"],
        "rhizome_onboard",
        json!({}),
        Duration::from_secs(10),
    )?;
    serde_json::from_str(&text).context("Failed to parse rhizome_onboard response")
}

fn run_rhizome_summary(project_root: &Path) -> Result<String> {
    let info = discover(Tool::Rhizome).context("Rhizome binary not found")?;
    let output = Command::new(&info.binary_path)
        .arg("summarize")
        .arg("--project")
        .arg(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute rhizome summarize")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "rhizome summarize failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn call_tool_text(
    tool: Tool,
    serve_args: &[&str],
    tool_name: &str,
    arguments: Value,
    timeout: Duration,
) -> Result<String> {
    let info = discover(tool).ok_or_else(|| anyhow!("{tool_name} requires {:?} on PATH", tool))?;

    let request = Request::new(
        "tools/call",
        json!({
            "name": tool_name,
            "arguments": arguments,
        }),
    );
    let request_str =
        serde_json::to_string(&request).context("Failed to serialize JSON-RPC request")? + "\n";

    let mut child = Command::new(&info.binary_path)
        .args(serve_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to spawn {}", info.binary_path.display()))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request_str.as_bytes())
            .context("Failed to write JSON-RPC request")?;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let mut stdout = child
        .stdout
        .take()
        .context("Failed to capture MCP stdout")?;
    std::thread::spawn(move || {
        let mut response = String::new();
        let _ = std::io::Read::read_to_string(&mut stdout, &mut response);
        let _ = tx.send(response);
    });

    let response = rx
        .recv_timeout(timeout)
        .with_context(|| format!("{tool_name} timed out"))?;
    let _ = child.wait();

    parse_tool_response(&response)
}

fn parse_tool_response(response: &str) -> Result<String> {
    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let json: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(error) = json.get("error") {
            anyhow::bail!("tool returned JSON-RPC error: {error}");
        }

        if let Some(result) = json.get("result") {
            if result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let message = result
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("tool call failed");
                anyhow::bail!("{message}");
            }

            if let Some(text) = result
                .get("content")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("text"))
                .and_then(Value::as_str)
            {
                return Ok(text.to_string());
            }
        }
    }

    anyhow::bail!("No valid JSON-RPC response received")
}

fn prompt_yes_no(prompt: &str, default_yes: bool) -> Result<PromptDecision> {
    if ONBOARD_CANCELLED.load(Ordering::SeqCst) {
        return Ok(PromptDecision::Cancelled);
    }

    let default = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("{} {} ", prompt.bold(), default.dimmed());
    io::stdout().flush().context("Failed to flush prompt")?;

    let mut input = String::new();
    let bytes = io::stdin()
        .lock()
        .read_line(&mut input)
        .context("Failed to read user input")?;

    if ONBOARD_CANCELLED.load(Ordering::SeqCst) {
        return Ok(PromptDecision::Cancelled);
    }

    if bytes == 0 {
        return Ok(if default_yes {
            PromptDecision::Yes
        } else {
            PromptDecision::No
        });
    }

    let response = input.trim().to_ascii_lowercase();
    if response.is_empty() {
        Ok(if default_yes {
            PromptDecision::Yes
        } else {
            PromptDecision::No
        })
    } else if matches!(response.as_str(), "y" | "yes") {
        Ok(PromptDecision::Yes)
    } else if matches!(response.as_str(), "n" | "no") {
        Ok(PromptDecision::No)
    } else {
        println!("Please answer y or n.");
        prompt_yes_no(prompt, default_yes)
    }
}

fn prompt_text(prompt: &str) -> Result<PromptText> {
    if ONBOARD_CANCELLED.load(Ordering::SeqCst) {
        return Ok(PromptText::Cancelled);
    }

    println!("{}", prompt.bold());
    print!("{}", "> ".dimmed());
    io::stdout().flush().context("Failed to flush prompt")?;

    let mut input = String::new();
    let bytes = io::stdin()
        .lock()
        .read_line(&mut input)
        .context("Failed to read user input")?;

    if ONBOARD_CANCELLED.load(Ordering::SeqCst) {
        return Ok(PromptText::Cancelled);
    }

    if bytes == 0 {
        return Ok(PromptText::Skipped);
    }

    let response = input.trim();
    if response.is_empty() {
        Ok(PromptText::Skipped)
    } else {
        Ok(PromptText::Value(response.to_string()))
    }
}

fn finish_cancelled() -> Result<()> {
    println!();
    println!(
        "{}",
        "Onboarding cancelled. No further changes were applied.".yellow()
    );
    Ok(())
}

fn print_step_heading(step: usize, title: &str) {
    println!(
        "{} {}",
        format!("Step {step}").bright_blue().bold(),
        title.bold()
    );
}

fn print_tool_status(status: &ToolStatus) {
    let marker = if status.installed {
        "ok".green().bold()
    } else {
        "--".yellow().bold()
    };
    println!("{} {:<8} {}", marker, status.name, status.detail);
}

fn print_summary_line(label: &str, value: &str) {
    println!("{} {}", format!("{label}:").bold(), value);
}

fn excerpt_lines(text: &str, max_lines: usize) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() <= max_lines {
        return text.to_string();
    }

    let mut excerpt = lines[..max_lines].join("\n");
    excerpt.push_str(&format!("\n... ({} more lines)", lines.len() - max_lines));
    excerpt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_codex_servers_adds_missing_entries() {
        let hyphae = ToolStatus {
            name: "Hyphae",
            installed: true,
            detail: "/usr/local/bin/hyphae".to_string(),
        };
        let rhizome = ToolStatus {
            name: "Rhizome",
            installed: true,
            detail: "/usr/local/bin/rhizome".to_string(),
        };

        let (updated, result) = upsert_codex_servers("", &hyphae, &rhizome).unwrap();
        assert_eq!(
            result.added,
            vec!["hyphae".to_string(), "rhizome".to_string()]
        );
        let parsed: toml::Value = toml::from_str(&updated).unwrap();
        let servers = parsed
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .unwrap();
        let hyphae_server = servers
            .get("hyphae")
            .and_then(toml::Value::as_table)
            .unwrap();
        let rhizome_server = servers
            .get("rhizome")
            .and_then(toml::Value::as_table)
            .unwrap();
        assert_eq!(
            hyphae_server.get("command").and_then(toml::Value::as_str),
            Some("/usr/local/bin/hyphae")
        );
        assert_eq!(
            hyphae_server.get("args").and_then(toml::Value::as_array),
            Some(&vec![toml::Value::String("serve".to_string())])
        );
        assert_eq!(
            rhizome_server.get("command").and_then(toml::Value::as_str),
            Some("/usr/local/bin/rhizome")
        );
        assert_eq!(
            rhizome_server.get("args").and_then(toml::Value::as_array),
            Some(&vec![
                toml::Value::String("serve".to_string()),
                toml::Value::String("--expanded".to_string()),
            ])
        );
    }

    #[test]
    fn test_upsert_codex_servers_preserves_existing_entries() {
        let existing = r#"
[model]
name = "gpt-5"

[mcp_servers.hyphae]
command = "/usr/local/bin/hyphae"
args = ["serve"]
"#;
        let hyphae = ToolStatus {
            name: "Hyphae",
            installed: true,
            detail: "/usr/local/bin/hyphae".to_string(),
        };
        let rhizome = ToolStatus {
            name: "Rhizome",
            installed: false,
            detail: "not found".to_string(),
        };

        let (updated, result) = upsert_codex_servers(existing, &hyphae, &rhizome).unwrap();
        assert!(updated.contains("[model]"));
        assert_eq!(result.added, Vec::<String>::new());
        assert_eq!(result.already_present, vec!["hyphae".to_string()]);
    }

    #[test]
    fn test_excerpt_lines_truncates_long_output() {
        let text = "one\ntwo\nthree\nfour";
        assert_eq!(excerpt_lines(text, 2), "one\ntwo\n... (2 more lines)");
        assert_eq!(excerpt_lines(text, 10), text);
    }

    #[test]
    fn test_find_workspace_cap_detects_sibling_repo() {
        let temp = tempfile::TempDir::new().unwrap();
        let workspace = temp.path();
        let mycelium = workspace.join("mycelium");
        let cap = workspace.join("cap");
        fs::create_dir_all(&mycelium).unwrap();
        fs::create_dir_all(&cap).unwrap();
        fs::write(cap.join("package.json"), "{}").unwrap();

        let detected = find_workspace_cap(&mycelium).unwrap();
        assert_eq!(detected, cap);
    }

    #[test]
    fn test_host_setup_plan_global() {
        assert_eq!(
            host_setup_plan(true),
            HostSetupPlan {
                init_command: "mycelium init -g",
                scope_label: "global",
            }
        );
    }

    #[test]
    fn test_host_setup_plan_project_local() {
        assert_eq!(
            host_setup_plan(false),
            HostSetupPlan {
                init_command: "mycelium init",
                scope_label: "project-local",
            }
        );
    }
}
