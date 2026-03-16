//! MCP client for Hyphae — stores large command output as retrievable chunks.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Summary returned by Hyphae after chunking command output.
#[derive(Debug, Deserialize)]
pub struct ChunkSummary {
    pub summary: String,
    pub document_id: String,
    #[allow(dead_code)]
    pub chunk_count: usize,
}

/// Store command output in Hyphae's chunked storage.
///
/// Spawns `hyphae serve` as a single-shot MCP subprocess, sends a `tools/call`
/// JSON-RPC request for `hyphae_store_command_output`, and parses the response.
///
/// Returns `Err` on any failure (timeout, parse error, Hyphae crash) — caller
/// should fall back to local filtering.
pub fn store_output(command: &str, output: &str, project: Option<&str>) -> Result<ChunkSummary> {
    let hyphae_bin = crate::hyphae::hyphae_binary().context("Hyphae binary not found")?;

    let project_name = project
        .map(|s| s.to_string())
        .unwrap_or_else(detect_project_name);

    let request = build_request(command, output, &project_name);

    // Spawn hyphae serve subprocess
    let mut child = Command::new(hyphae_bin)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn hyphae serve")?;

    // Write request to stdin, then close it
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request.as_bytes())
            .context("Failed to write to hyphae stdin")?;
    }

    // Read response with 5-second timeout
    let (tx, rx) = mpsc::channel();
    let mut stdout = child.stdout.take().context("Failed to get hyphae stdout")?;

    std::thread::spawn(move || {
        let mut response = String::new();
        let _ = stdout.read_to_string(&mut response);
        let _ = tx.send(response);
    });

    let response = rx
        .recv_timeout(Duration::from_secs(5))
        .context("Hyphae response timed out after 5 seconds")?;

    // Clean up child process
    let _ = child.wait();

    parse_response(&response)
}

fn build_request(command: &str, output: &str, project: &str) -> String {
    let request = spore::jsonrpc::Request::new(
        "tools/call",
        serde_json::json!({
            "name": "hyphae_store_command_output",
            "arguments": {
                "command": command,
                "output": output,
                "project": project,
            }
        }),
    );
    // Use line-delimited format (not Content-Length) since hyphae uses that
    serde_json::to_string(&request).expect("Request serialization cannot fail") + "\n"
}

fn parse_response(response: &str) -> Result<ChunkSummary> {
    // Find the first complete JSON object in the response
    // (hyphae may emit initialization messages before the response)
    for line in response.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            // Look for the result.content[0].text field
            if let Some(text) = json
                .get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
            {
                let summary: ChunkSummary = serde_json::from_str(text)
                    .context("Failed to parse ChunkSummary from Hyphae response")?;
                return Ok(summary);
            }

            // Check for JSON-RPC error
            if let Some(error) = json.get("error") {
                bail!("Hyphae returned error: {}", error);
            }
        }
    }

    bail!("No valid JSON-RPC response found in Hyphae output")
}

fn detect_project_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_request_format() {
        let request = build_request("cargo test", "test output here", "mycelium");
        let parsed: serde_json::Value = serde_json::from_str(&request).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "tools/call");
        assert_eq!(parsed["params"]["name"], "hyphae_store_command_output");
        assert_eq!(parsed["params"]["arguments"]["command"], "cargo test");
        assert_eq!(parsed["params"]["arguments"]["output"], "test output here");
        assert_eq!(parsed["params"]["arguments"]["project"], "mycelium");
    }

    #[test]
    fn test_parse_response_valid() {
        let response = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"summary\":\"5 tests passed\",\"document_id\":\"abc123\",\"chunk_count\":3}"}]}}"#;
        let summary = parse_response(response).unwrap();
        assert_eq!(summary.summary, "5 tests passed");
        assert_eq!(summary.document_id, "abc123");
        assert_eq!(summary.chunk_count, 3);
    }

    #[test]
    fn test_parse_response_with_prefix_messages() {
        let response = "some init message\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"{\\\"summary\\\":\\\"ok\\\",\\\"document_id\\\":\\\"def456\\\",\\\"chunk_count\\\":1}\"}]}}";
        let summary = parse_response(response).unwrap();
        assert_eq!(summary.document_id, "def456");
    }

    #[test]
    fn test_parse_response_error() {
        let response =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid request"}}"#;
        let result = parse_response(response);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("error"));
    }

    #[test]
    fn test_parse_response_empty() {
        let result = parse_response("");
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_project_name() {
        let name = detect_project_name();
        assert!(!name.is_empty());
    }
}
