// ─────────────────────────────────────────────────────────────────────────────
// Context gathering command
// ─────────────────────────────────────────────────────────────────────────────
//
// `mycelium context <task>` — gathers relevant context from Hyphae and
// formats it as a compact briefing for piping into LLM workflows.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use spore::logging::{SpanContext, subprocess_span, tool_span};
use spore::{EcosystemError, Tool};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

pub fn run(
    task: &str,
    project: Option<&str>,
    budget: u64,
    include: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let hyphae_bin = crate::hyphae::hyphae_binary();
    let hyphae_bin = match hyphae_bin {
        Some(bin) => bin,
        None => bail!("Hyphae binary not found. Install hyphae or add it to PATH."),
    };

    let response = call_gather_context(hyphae_bin, task, project, budget, include)?;

    if json_output {
        println!("{response}");
    } else {
        let briefing = format_briefing(task, &response)?;
        print!("{briefing}");
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// MCP subprocess communication
// ─────────────────────────────────────────────────────────────────────────────

fn call_gather_context(
    hyphae_bin: &str,
    task: &str,
    project: Option<&str>,
    budget: u64,
    include: Option<&str>,
) -> Result<String> {
    let span_context = match std::env::current_dir() {
        Ok(path) => {
            SpanContext::for_app("mycelium").with_workspace_root(path.display().to_string())
        }
        Err(_) => SpanContext::for_app("mycelium"),
    };
    let arguments = build_gather_context_arguments(task, project, budget, include);
    let _tool_span = tool_span("hyphae_gather_context", &span_context).entered();

    let request = spore::jsonrpc::Request::new(
        "tools/call",
        json!({
            "name": "hyphae_gather_context",
            "arguments": arguments,
        }),
    );

    let request_str =
        serde_json::to_string(&request).context("Failed to serialize request")? + "\n";

    // Spawn hyphae serve subprocess
    let _spawn_span = subprocess_span("hyphae serve", &span_context).entered();
    let mut child = Command::new(hyphae_bin)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn hyphae serve")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request_str.as_bytes())
            .context("Failed to write to hyphae stdin")?;
    }

    // Read response with 10-second timeout
    let (tx, rx) = mpsc::channel();
    let mut stdout = child.stdout.take().context("Failed to get hyphae stdout")?;

    std::thread::spawn(move || {
        let mut response = String::new();
        let _ = stdout.read_to_string(&mut response);
        let _ = tx.send(response);
    });

    let response = rx
        .recv_timeout(Duration::from_secs(10))
        .context("Hyphae response timed out after 10 seconds")?;
    debug!("Received response from hyphae gather_context subprocess");

    let _ = child.wait();

    parse_mcp_response(&response)
}

fn build_gather_context_arguments(
    task: &str,
    project: Option<&str>,
    budget: u64,
    include: Option<&str>,
) -> Value {
    let mut arguments = serde_json::Map::from_iter([
        ("task".to_string(), json!(task)),
        ("token_budget".to_string(), json!(budget)),
    ]);

    if let Some(proj) = project {
        arguments.insert("project".to_string(), json!(proj));

        let detected = crate::hyphae_client::detect_project_identity();
        if detected.project == proj
            && let (Some(project_root), Some(worktree_id)) = (
                detected.project_root.as_deref(),
                detected.worktree_id.as_deref(),
            )
        {
            arguments.insert("project_root".to_string(), json!(project_root));
            arguments.insert("worktree_id".to_string(), json!(worktree_id));
        }
    }

    if let Some(inc) = include {
        let sources: Vec<&str> = inc.split(',').map(|s| s.trim()).collect();
        arguments.insert("include".to_string(), json!(sources));
    }

    Value::Object(arguments)
}

fn parse_mcp_response(response: &str) -> Result<String> {
    for line in response.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(error) = json.get("error") {
                warn!("Hyphae gather_context returned JSON-RPC error: {error}");
                bail!(
                    "{}",
                    EcosystemError::new(
                        Tool::Hyphae,
                        "jsonrpc_error",
                        format!("Hyphae returned error: {error}")
                    )
                    .to_json_string()
                );
            }

            if let Some(result) = json.get("result") {
                if result_is_error(result) {
                    let message = first_tool_result_text(result)
                        .filter(|text| !text.trim().is_empty())
                        .unwrap_or("Hyphae tool call failed");
                    warn!("Hyphae gather_context returned tool error: {message}");
                    bail!(
                        "{}",
                        EcosystemError::new(
                            Tool::Hyphae,
                            "tool_error",
                            format!("Hyphae returned tool error: {message}")
                        )
                        .to_json_string()
                    );
                }

                if let Some(text) = first_tool_result_text(result) {
                    return Ok(text.to_string());
                }
            }
        }
    }

    bail!(
        "{}",
        EcosystemError::new(
            Tool::Hyphae,
            "invalid_response",
            "No valid JSON-RPC response from Hyphae"
        )
        .to_json_string()
    )
}

fn result_is_error(result: &serde_json::Value) -> bool {
    result
        .get("isError")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn first_tool_result_text(result: &serde_json::Value) -> Option<&str> {
    result
        .get("content")
        .and_then(|content| content.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
}

// ─────────────────────────────────────────────────────────────────────────────
// Briefing formatter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GatherResponse {
    context: Vec<ContextEntry>,
    tokens_used: u64,
    tokens_budget: u64,
    sources_queried: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ContextEntry {
    source: String,
    content: String,
    relevance: f64,
    topic: Option<String>,
    symbol: Option<String>,
}

fn format_briefing(task: &str, raw_json: &str) -> Result<String> {
    let resp: GatherResponse =
        serde_json::from_str(raw_json).context("Failed to parse gather_context response")?;

    let mut out = String::new();

    out.push_str(&format!("Context Briefing for: \"{task}\"\n"));
    out.push_str(&"\u{2500}".repeat(45));
    out.push('\n');
    out.push('\n');

    // Group by source
    let memories: Vec<&ContextEntry> = resp
        .context
        .iter()
        .filter(|e| e.source == "memory")
        .collect();
    let errors: Vec<&ContextEntry> = resp
        .context
        .iter()
        .filter(|e| e.source == "error")
        .collect();
    let sessions: Vec<&ContextEntry> = resp
        .context
        .iter()
        .filter(|e| e.source == "session")
        .collect();
    let code: Vec<&ContextEntry> = resp.context.iter().filter(|e| e.source == "code").collect();

    if !memories.is_empty() {
        out.push_str(&format!("Relevant Memories ({}):\n", memories.len()));
        for entry in &memories {
            let level = relevance_label(entry.relevance);
            let topic_suffix = entry
                .topic
                .as_deref()
                .map(|t| format!(" ({t})"))
                .unwrap_or_default();
            let summary = truncate_line(&entry.content, 80);
            out.push_str(&format!("  [{level}] {summary}{topic_suffix}\n"));
        }
        out.push('\n');
    }

    if !code.is_empty() {
        out.push_str(&format!("Related Code ({}):\n", code.len()));
        for entry in &code {
            let symbol = entry.symbol.as_deref().unwrap_or("?");
            let summary = truncate_line(&entry.content, 60);
            out.push_str(&format!("  {symbol}: {summary}\n"));
        }
        out.push('\n');
    }

    if !errors.is_empty() {
        out.push_str("Past Errors:\n");
        for entry in &errors {
            let summary = truncate_line(&entry.content, 80);
            out.push_str(&format!("  Fixed: {summary}\n"));
        }
        out.push('\n');
    }

    if !sessions.is_empty() {
        out.push_str(&format!("Recent Sessions ({}):\n", sessions.len()));
        for entry in &sessions {
            let summary = truncate_line(&entry.content, 80);
            out.push_str(&format!("  {summary}\n"));
        }
        out.push('\n');
    }

    if resp.context.is_empty() {
        out.push_str("No relevant context found.\n\n");
    }

    out.push_str(&format!(
        "Budget: {}/{} tokens | Sources: {}\n",
        resp.tokens_used,
        resp.tokens_budget,
        resp.sources_queried.join(", ")
    ));

    Ok(out)
}

fn relevance_label(score: f64) -> &'static str {
    if score >= 0.8 {
        "high"
    } else if score >= 0.5 {
        "medium"
    } else {
        "low"
    }
}

fn truncate_line(s: &str, max: usize) -> String {
    let line = s.lines().next().unwrap_or(s);
    if line.len() > max {
        format!("{}...", &line[..max])
    } else {
        line.to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_briefing_empty() {
        let json = r#"{"context":[],"tokens_used":0,"tokens_budget":2000,"sources_queried":["memories","errors"]}"#;
        let briefing = format_briefing("test task", json).unwrap();
        assert!(briefing.contains("Context Briefing for: \"test task\""));
        assert!(briefing.contains("No relevant context found."));
        assert!(briefing.contains("0/2000 tokens"));
    }

    #[test]
    fn test_format_briefing_with_results() {
        let json = r#"{
            "context": [
                {"source": "memory", "topic": "auth", "content": "JWT with RS256", "relevance": 0.95},
                {"source": "code", "symbol": "AuthMiddleware", "content": "pub struct AuthMiddleware", "relevance": 0.7},
                {"source": "error", "topic": "errors/resolved", "content": "Fixed lifetime mismatch", "relevance": 0.6}
            ],
            "tokens_used": 150,
            "tokens_budget": 2000,
            "sources_queried": ["memories", "errors", "code"]
        }"#;
        let briefing = format_briefing("refactor auth", json).unwrap();
        assert!(briefing.contains("Relevant Memories (1):"));
        assert!(briefing.contains("[high] JWT with RS256"));
        assert!(briefing.contains("Related Code (1):"));
        assert!(briefing.contains("AuthMiddleware:"));
        assert!(briefing.contains("Past Errors:"));
        assert!(briefing.contains("Fixed: Fixed lifetime mismatch"));
        assert!(briefing.contains("150/2000 tokens"));
    }

    #[test]
    fn test_relevance_label() {
        assert_eq!(relevance_label(0.95), "high");
        assert_eq!(relevance_label(0.8), "high");
        assert_eq!(relevance_label(0.6), "medium");
        assert_eq!(relevance_label(0.3), "low");
    }

    #[test]
    fn test_truncate_line_short() {
        assert_eq!(truncate_line("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_line_long() {
        let long = "a".repeat(100);
        let truncated = truncate_line(&long, 20);
        assert_eq!(truncated.len(), 23); // 20 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_line_multiline() {
        let multi = "first line\nsecond line\nthird line";
        assert_eq!(truncate_line(multi, 80), "first line");
    }

    #[test]
    fn test_parse_mcp_response_valid() {
        let response = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"context\":[],\"tokens_used\":0,\"tokens_budget\":2000,\"sources_queried\":[]}"}]}}"#;
        let result = parse_mcp_response(response).unwrap();
        assert!(result.contains("context"));
    }

    #[test]
    fn test_parse_mcp_response_tool_error_takes_priority_over_payload_text() {
        let response = r#"{"jsonrpc":"2.0","id":1,"result":{"isError":true,"content":[{"type":"text","text":"{\"context\":[],\"tokens_used\":0,\"tokens_budget\":2000,\"sources_queried\":[]}"}]}}"#;
        let err = parse_mcp_response(response).unwrap_err();
        let payload: serde_json::Value =
            serde_json::from_str(&err.to_string()).expect("ecosystem error json");
        assert_eq!(payload["tool"].as_str(), Some("hyphae"));
        assert_eq!(payload["code"].as_str(), Some("tool_error"));
    }

    #[test]
    fn test_parse_mcp_response_error() {
        let response =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid request"}}"#;
        let result = parse_mcp_response(response);
        assert!(result.is_err());
        let payload: serde_json::Value =
            serde_json::from_str(&result.unwrap_err().to_string()).expect("ecosystem error json");
        assert_eq!(payload["code"].as_str(), Some("jsonrpc_error"));
    }

    #[test]
    fn test_build_gather_context_arguments_stays_global_without_project() {
        let arguments = build_gather_context_arguments("login", None, 500, Some("memories"));

        assert_eq!(arguments["task"], "login");
        assert_eq!(arguments["token_budget"], 500);
        assert!(arguments.get("project").is_none());
        assert!(arguments.get("project_root").is_none());
        assert!(arguments.get("worktree_id").is_none());
        assert_eq!(arguments["include"], json!(["memories"]));
    }

    #[test]
    fn test_build_gather_context_arguments_adds_identity_for_matching_project() {
        let detected = crate::hyphae_client::detect_project_identity();
        if let (Some(project_root), Some(worktree_id)) = (
            detected.project_root.as_deref(),
            detected.worktree_id.as_deref(),
        ) {
            let arguments =
                build_gather_context_arguments("login", Some(&detected.project), 500, None);

            assert_eq!(arguments["project"], detected.project);
            assert_eq!(arguments["project_root"], project_root);
            assert_eq!(arguments["worktree_id"], worktree_id);
        }
    }

    #[test]
    fn test_build_gather_context_arguments_omits_identity_for_mismatched_project() {
        let arguments = build_gather_context_arguments("login", Some("other-project"), 500, None);

        assert_eq!(arguments["project"], "other-project");
        assert!(arguments.get("project_root").is_none());
        assert!(arguments.get("worktree_id").is_none());
    }
}
