//! CLI client for Rhizome — code intelligence via tree-sitter and LSP.

use anyhow::{Context, Result, bail};
use spore::McpClient;
use spore::logging::{SpanContext, subprocess_span, tool_span};
use std::path::Path;
use std::time::Duration;

/// Get hierarchical outline (modules, classes, methods with nesting) for a file.
pub fn get_structure(file: &Path) -> Result<String> {
    let context = span_context("structure", file);
    let _tool_span = tool_span("rhizome_structure", &context).entered();

    let file_str = file.to_str().context("Invalid file path encoding")?;

    let mut client = McpClient::spawn(spore::Tool::Rhizome, &[])
        .context("Failed to start rhizome MCP server")?
        .with_timeout(Duration::from_secs(3));

    let command_label = format!("rhizome get_structure {file_str}");
    let _subprocess_span = subprocess_span(&command_label, &context).entered();

    let result = client
        .call_tool("get_structure", serde_json::json!({ "file": file_str }))
        .context("Rhizome get_structure failed")?;

    // get_structure returns: [{"type":"text","text":"<indented tree text>"}]
    let text = result
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Unexpected get_structure response shape"))?;

    if text.is_empty() {
        bail!(
            "rhizome get_structure returned empty output for {}",
            file_str
        );
    }

    Ok(text.trim_end().to_string())
}

fn span_context(subcommand: &str, file: &Path) -> SpanContext {
    let context = SpanContext::for_app("mycelium").with_tool(format!("rhizome_{subcommand}"));
    match file.parent() {
        Some(parent) => context.with_workspace_root(parent.display().to_string()),
        None => context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_structure_without_rhizome() {
        let path = PathBuf::from("src/main.rs");
        let result = get_structure(&path);
        if let Err(err) = result {
            let msg = err.to_string();
            assert!(
                msg.contains("not found")
                    || msg.contains("timed out")
                    || msg.contains("failed")
                    || msg.contains("MCP"),
                "Unexpected error: {}",
                msg
            );
        }
    }

    #[test]
    fn test_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/file.rs");
        let result = get_structure(&path);
        // Should fail gracefully
        assert!(result.is_err());
    }

    #[test]
    #[ignore]
    fn test_get_structure_with_rhizome_returns_text() {
        let path = PathBuf::from("src/rhizome_client.rs");
        let result = get_structure(&path);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        let text = result.unwrap();
        assert!(!text.is_empty());
    }
}
