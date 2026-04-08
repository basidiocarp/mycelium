//! MCP client for Hyphae — stores large command output as retrievable chunks.
//!
//! Uses a persistent `hyphae serve` subprocess to avoid cold start overhead for
//! each call. The subprocess is spawned on first use and reused for all
//! subsequent `store_output()` calls. If the subprocess crashes, a new one is
//! automatically spawned on the next call.

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use spore::{EcosystemError, McpClient, Tool};

const COMMAND_OUTPUT_SCHEMA_VERSION: &str = "1.0";

/// Summary returned by Hyphae after chunking command output.
#[derive(Debug, Deserialize)]
pub struct ChunkSummary {
    pub summary: String,
    pub document_id: String,
    #[allow(dead_code)]
    pub chunk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectIdentity {
    pub project: String,
    pub project_root: Option<String>,
    pub worktree_id: Option<String>,
}

/// ─────────────────────────────────────────────────────────────────────────────
/// Persistent Hyphae MCP connection
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Cached Hyphae process — initialized on first use, reused for all subsequent
/// calls. If the subprocess crashes, a new one is spawned on the next call.
static HYPHAE_PROCESS: Mutex<Option<McpClient>> = Mutex::new(None);

/// Get or establish the persistent Hyphae connection.
///
/// Checks if a connection exists and is alive. If not, attempts to create a new
/// one. Returns `Err` if initialization fails (not a panic, since Hyphae is
/// optional).
fn get_or_connect() -> Result<MutexGuard<'static, Option<McpClient>>> {
    let mut guard = HYPHAE_PROCESS
        .lock()
        .map_err(|e| anyhow!("Hyphae lock poisoned: {e}"))?;

    if let Some(client) = guard.as_mut()
        && client.is_alive()
    {
        return Ok(guard);
    }

    match McpClient::spawn(Tool::Hyphae, &["serve"]) {
        Ok(client) => {
            *guard = Some(client.with_timeout(Duration::from_secs(10)));
            Ok(guard)
        }
        Err(e) => {
            *guard = None;
            Err(anyhow!(
                "{}",
                EcosystemError::new(Tool::Hyphae, "spawn_failed", "Failed to spawn hyphae serve")
                    .with_cause(EcosystemError::from_spore_error(Tool::Hyphae, &e))
                    .to_json_string()
            ))
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
    let identity = project
        .map(|name| ProjectIdentity {
            project: name.to_string(),
            project_root: None,
            worktree_id: None,
        })
        .unwrap_or_else(detect_project_identity);
    let runtime_session_id = current_runtime_session_id();

    let arguments = build_arguments(command, output, &identity, runtime_session_id.as_deref());

    let mut guard = get_or_connect()?;

    let client = guard
        .as_mut()
        .expect("connection should exist after get_or_connect");

    match client.call_tool("hyphae_store_command_output", arguments) {
        Ok(response) => parse_response(&response),
        Err(e) => {
            *guard = None;
            Err(anyhow!(
                "{}",
                EcosystemError::new(
                    Tool::Hyphae,
                    "call_tool_failed",
                    "Failed to call hyphae_store_command_output"
                )
                .with_cause(EcosystemError::from_spore_error(Tool::Hyphae, &e))
                .to_json_string()
            ))
        }
    }
}

fn build_arguments(
    command: &str,
    output: &str,
    identity: &ProjectIdentity,
    runtime_session_id: Option<&str>,
) -> Value {
    let mut arguments = serde_json::Map::from_iter([
        (
            "schema_version".to_string(),
            json!(COMMAND_OUTPUT_SCHEMA_VERSION),
        ),
        ("command".to_string(), json!(command)),
        ("output".to_string(), json!(output)),
        ("project".to_string(), json!(identity.project)),
    ]);
    if let (Some(project_root), Some(worktree_id)) = (
        identity.project_root.as_deref(),
        identity.worktree_id.as_deref(),
    ) {
        arguments.insert("project_root".to_string(), json!(project_root));
        arguments.insert("worktree_id".to_string(), json!(worktree_id));
    }
    if let Some(runtime_session_id) = runtime_session_id {
        arguments.insert("runtime_session_id".to_string(), json!(runtime_session_id));
    }

    Value::Object(arguments)
}

fn current_runtime_session_id() -> Option<String> {
    spore::claude_session_id()
}

/// ─────────────────────────────────────────────────────────────────────────────
/// Response parsing
/// ─────────────────────────────────────────────────────────────────────────────
///
fn parse_response(json: &serde_json::Value) -> Result<ChunkSummary> {
    // Check for JSON-RPC error
    if let Some(error) = json.get("error") {
        return Err(hyphae_protocol_error(
            "jsonrpc_error",
            format!("Hyphae returned error: {error}"),
        ));
    }

    if let Some(result) = json.get("result") {
        if result_is_error(result) {
            let message = first_tool_result_text(result)
                .filter(|text| !text.trim().is_empty())
                .unwrap_or("Hyphae tool call failed");
            return Err(hyphae_protocol_error(
                "tool_error",
                format!("Hyphae returned tool error: {message}"),
            ));
        }

        if let Some(text) = first_tool_result_text(result) {
            let summary: ChunkSummary = serde_json::from_str(text).map_err(|error| {
                anyhow!(
                    "{}",
                    EcosystemError::new(
                        Tool::Hyphae,
                        "parse_error",
                        "Failed to parse ChunkSummary from Hyphae response"
                    )
                    .with_cause(EcosystemError::new(
                        Tool::Hyphae,
                        "json_error",
                        error.to_string()
                    ))
                    .to_json_string()
                )
            })?;
            return Ok(summary);
        }
    }

    Err(hyphae_protocol_error(
        "invalid_response",
        "No valid response in Hyphae JSON-RPC message",
    ))
}

fn hyphae_protocol_error(code: &str, message: impl Into<String>) -> anyhow::Error {
    anyhow!(
        "{}",
        EcosystemError::new(Tool::Hyphae, code, message).to_json_string()
    )
}

pub(crate) fn detect_project_identity() -> ProjectIdentity {
    std::env::current_dir()
        .ok()
        .map(|path| detect_project_identity_from(&path))
        .unwrap_or_else(|| ProjectIdentity {
            project: "unknown".to_string(),
            project_root: None,
            worktree_id: None,
        })
}

fn detect_project_identity_from(path: &Path) -> ProjectIdentity {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let repo_root = canonical
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_path_buf);
    let project = repo_root
        .as_deref()
        .and_then(path_name)
        .or_else(|| path_name(&canonical))
        .unwrap_or_else(|| "unknown".to_string());

    let (project_root, worktree_id) = repo_root
        .as_deref()
        .and_then(identity_v1_fields)
        .unwrap_or((None, None));

    ProjectIdentity {
        project,
        project_root,
        worktree_id,
    }
}

fn path_name(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
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

fn identity_v1_fields(repo_root: &Path) -> Option<(Option<String>, Option<String>)> {
    let canonical_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let git_marker = canonical_root.join(".git");
    let worktree_id = git_worktree_id(&git_marker)?;

    Some((
        Some(canonical_root.to_string_lossy().into_owned()),
        Some(worktree_id),
    ))
}

fn git_worktree_id(git_marker: &Path) -> Option<String> {
    let git_dir = git_dir_from_marker(git_marker)?;
    std::fs::read_to_string(git_dir.join("HEAD")).ok()?;

    match (
        git_dir.file_name().and_then(|name| name.to_str()),
        git_dir
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str()),
    ) {
        (Some(".git"), _) => Some("main".to_string()),
        (Some(name), Some("worktrees")) => Some(name.to_string()),
        _ => Some(git_dir.to_string_lossy().into_owned()),
    }
}

fn git_dir_from_marker(git_marker: &Path) -> Option<PathBuf> {
    if git_marker.is_dir() {
        return Some(
            git_marker
                .canonicalize()
                .unwrap_or_else(|_| git_marker.to_path_buf()),
        );
    }

    let git_file = std::fs::read_to_string(git_marker).ok()?;
    let git_dir = git_file.strip_prefix("gitdir:")?.trim();
    let git_dir_path = if Path::new(git_dir).is_absolute() {
        Path::new(git_dir).to_path_buf()
    } else {
        git_marker.parent()?.join(git_dir)
    };

    Some(
        git_dir_path
            .canonicalize()
            .unwrap_or_else(|_| git_dir_path.to_path_buf()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_build_arguments_format() {
        let identity = ProjectIdentity {
            project: "mycelium".to_string(),
            project_root: Some("/repo/mycelium".to_string()),
            worktree_id: Some("wt-alpha".to_string()),
        };
        let arguments = build_arguments("cargo test", "test output here", &identity, None);

        assert_eq!(arguments["schema_version"], "1.0");
        assert_eq!(arguments["command"], "cargo test");
        assert_eq!(arguments["output"], "test output here");
        assert_eq!(arguments["project"], "mycelium");
        assert_eq!(arguments["project_root"], "/repo/mycelium");
        assert_eq!(arguments["worktree_id"], "wt-alpha");
        assert!(arguments.get("runtime_session_id").is_none());
    }

    #[test]
    fn test_build_arguments_omits_partial_identity_fields() {
        let identity = ProjectIdentity {
            project: "mycelium".to_string(),
            project_root: None,
            worktree_id: None,
        };
        let arguments = build_arguments("cargo test", "test output here", &identity, None);

        assert_eq!(arguments["schema_version"], "1.0");
        assert_eq!(arguments["project"], "mycelium");
        assert!(arguments.get("project_root").is_none());
        assert!(arguments.get("worktree_id").is_none());
    }

    #[test]
    fn test_build_arguments_includes_runtime_session_id_when_present() {
        let identity = ProjectIdentity {
            project: "mycelium".to_string(),
            project_root: Some("/repo/mycelium".to_string()),
            worktree_id: Some("wt-alpha".to_string()),
        };
        let arguments = build_arguments(
            "cargo test",
            "test output here",
            &identity,
            Some("claude-session-42"),
        );

        assert_eq!(
            arguments["runtime_session_id"].as_str(),
            Some("claude-session-42")
        );
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
    fn test_parse_response_tool_error_takes_priority_over_payload_deserialize() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": true,
                "content": [{
                    "type": "text",
                    "text": r#"{"summary":"should not deserialize","document_id":"abc123","chunk_count":3}"#
                }]
            }
        });
        let err = parse_response(&json).unwrap_err();
        let payload: serde_json::Value =
            serde_json::from_str(&err.to_string()).expect("ecosystem error json");
        assert_eq!(payload["tool"].as_str(), Some("hyphae"));
        assert_eq!(payload["code"].as_str(), Some("tool_error"));
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
        let payload: serde_json::Value =
            serde_json::from_str(&result.unwrap_err().to_string()).expect("ecosystem error json");
        assert_eq!(payload["code"].as_str(), Some("jsonrpc_error"));
    }

    #[test]
    fn test_detect_project_identity_from_prefers_repo_root() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("mycelium");
        let nested = repo_root.join("src").join("init");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir(repo_root.join(".git")).unwrap();
        std::fs::write(repo_root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

        let identity = detect_project_identity_from(&nested);
        assert_eq!(identity.project, "mycelium");
        assert_eq!(
            identity.project_root.as_deref(),
            Some(repo_root.canonicalize().unwrap().to_string_lossy().as_ref())
        );
        assert_eq!(identity.worktree_id.as_deref(), Some("main"));
    }

    #[test]
    fn test_detect_project_identity_from_supports_git_file_marker() {
        let temp = tempdir().unwrap();
        let git_main = temp.path().join("git-main");
        let git_dir = git_main.join(".git/worktrees/wt-alpha");
        let repo_root = temp.path().join("linked-worktree");
        let nested = repo_root.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/wt-alpha\n").unwrap();
        std::fs::write(
            repo_root.join(".git"),
            format!("gitdir: {}\n", git_dir.display()),
        )
        .unwrap();

        let identity = detect_project_identity_from(&nested);
        assert_eq!(identity.project, "linked-worktree");
        assert_eq!(identity.worktree_id.as_deref(), Some("wt-alpha"));
    }

    #[test]
    fn test_detect_project_identity_from_falls_back_to_basename() {
        let temp = tempdir().unwrap();
        let leaf = temp.path().join("scratch");
        std::fs::create_dir_all(&leaf).unwrap();

        let identity = detect_project_identity_from(&leaf);
        assert_eq!(identity.project, "scratch");
        assert!(identity.project_root.is_none());
        assert!(identity.worktree_id.is_none());
    }
}
