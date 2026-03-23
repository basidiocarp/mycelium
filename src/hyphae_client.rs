//! MCP client for Hyphae — stores large command output as retrievable chunks.
//!
//! Uses a persistent `hyphae serve` subprocess to avoid cold start overhead for
//! each call. The subprocess is spawned on first use and reused for all
//! subsequent `store_output()` calls. If the subprocess crashes, a new one is
//! automatically spawned on the next call.

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Mutex, MutexGuard};

/// Summary returned by Hyphae after chunking command output.
#[derive(Debug, Deserialize)]
pub struct ChunkSummary {
    pub summary: String,
    pub document_id: String,
    #[allow(dead_code)]
    pub chunk_count: usize,
}

/// ─────────────────────────────────────────────────────────────────────────────
/// Persistent Hyphae MCP connection
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Cached Hyphae process — initialized on first use, reused for all subsequent
/// calls. If the subprocess crashes, a new one is spawned on the next call.
static HYPHAE_PROCESS: Mutex<Option<HyphaeConnection>> = Mutex::new(None);

struct HyphaeConnection {
    child: Child,
    stdout_reader: BufReader<ChildStdout>,
}

impl HyphaeConnection {
    /// Initialize persistent Hyphae MCP connection.
    fn init() -> Result<Self> {
        let hyphae_bin = crate::hyphae::hyphae_binary().context("Hyphae binary not found")?;

        let mut child = Command::new(hyphae_bin)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn hyphae serve")?;

        let stdout = child.stdout.take().context("Failed to get stdout pipe")?;

        let stdout_reader = BufReader::new(stdout);

        Ok(HyphaeConnection {
            child,
            stdout_reader,
        })
    }

    /// Check if the child process is still running.
    fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    /// Send a request and read the response line-by-line. Returns the parsed
    /// JSON-RPC response.
    fn call(&mut self, request: &str) -> Result<serde_json::Value> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .context("Lost stdin pipe to hyphae")?;

        stdin
            .write_all(request.as_bytes())
            .context("Failed to write to hyphae stdin")?;
        stdin.flush().context("Failed to flush hyphae stdin")?;

        // Read response line by line until we find valid JSON
        let mut line = String::new();
        loop {
            line.clear();
            self.stdout_reader
                .read_line(&mut line)
                .context("Failed to read from hyphae stdout")?;

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
                return Ok(json);
            }
        }
    }
}

/// Get or establish the persistent Hyphae connection.
///
/// Checks if a connection exists and is alive. If not, attempts to create a new
/// one. Returns `Err` if initialization fails (not a panic, since Hyphae is
/// optional).
fn get_or_connect() -> Result<MutexGuard<'static, Option<HyphaeConnection>>> {
    let mut guard = HYPHAE_PROCESS
        .lock()
        .map_err(|e| anyhow!("Hyphae lock poisoned: {e}"))?;

    // Check if we have a connection and it's still alive. Return if alive, otherwise
    // attempt to establish a new connection below.
    if let Some(conn) = guard.as_mut()
        && conn.is_alive()
    {
        return Ok(guard);
    }

    // Connection missing or dead — try to create a new one
    match HyphaeConnection::init() {
        Ok(conn) => {
            *guard = Some(conn);
            Ok(guard)
        }
        Err(e) => {
            *guard = None;
            Err(e)
        }
    }
}

/// ─────────────────────────────────────────────────────────────────────────────
/// Public API
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Store command output in Hyphae's chunked storage.
///
/// Uses a persistent MCP connection to avoid subprocess spawn overhead per call.
/// If the Hyphae subprocess crashes, it will be respawned on the next call.
///
/// Returns `Err` on any failure (timeout, parse error, Hyphae crash, etc.) —
/// caller should fall back to local filtering.
pub fn store_output(command: &str, output: &str, project: Option<&str>) -> Result<ChunkSummary> {
    let project_name = project
        .map(|s| s.to_string())
        .unwrap_or_else(detect_project_name);

    let request = build_request(command, output, &project_name);

    let mut guard = get_or_connect()?;

    // SAFETY: guard holds the lock and is Some after get_or_connect succeeds
    let conn = guard
        .as_mut()
        .expect("connection should exist after get_or_connect");

    match conn.call(&request) {
        Ok(response) => parse_response(&response),
        Err(e) => {
            // Drop the connection on call failure so next call reconnects
            *guard = None;
            Err(e)
        }
    }
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

/// ─────────────────────────────────────────────────────────────────────────────
/// Response parsing
/// ─────────────────────────────────────────────────────────────────────────────
///
fn parse_response(json: &serde_json::Value) -> Result<ChunkSummary> {
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

    bail!("No valid response in Hyphae JSON-RPC message")
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
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{
                    "type": "text",
                    "text": r#"{"summary":"5 tests passed","document_id":"abc123","chunk_count":3}"#
                }]
            }
        });
        let summary = parse_response(&json).unwrap();
        assert_eq!(summary.summary, "5 tests passed");
        assert_eq!(summary.document_id, "abc123");
        assert_eq!(summary.chunk_count, 3);
    }

    #[test]
    fn test_parse_response_error() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid request"
            }
        });
        let result = parse_response(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("error"));
    }

    #[test]
    fn test_detect_project_name() {
        let name = detect_project_name();
        assert!(!name.is_empty());
    }
}
