use super::shared::{codex_project_filter_matches, cutoff_time};
use super::{ExtractedCommand, SessionProvider};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Session provider that reads Codex JSONL files from `~/.codex/sessions/`.
pub struct CodexProvider;

impl CodexProvider {
    /// Get the base directory for Codex sessions.
    fn sessions_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let dir = home.join(".codex").join("sessions");
        if !dir.exists() {
            anyhow::bail!(
                "Codex sessions directory not found: {}\nMake sure Codex has been used at least once.",
                dir.display()
            );
        }
        Ok(dir)
    }

    /// Whether Codex history is available.
    pub fn history_root_exists() -> bool {
        dirs::home_dir()
            .map(|home| home.join(".codex").join("sessions").exists())
            .unwrap_or(false)
    }

    pub(super) fn discover_sessions_in(
        root: &Path,
        project_filter: Option<&str>,
        since_days: Option<u64>,
    ) -> Result<Vec<PathBuf>> {
        let cutoff = cutoff_time(since_days);
        let mut sessions = Vec::new();

        for walk_entry in WalkBuilder::new(root)
            .git_ignore(false)
            .follow_links(false)
            .build()
            .filter_map(|e| e.ok())
        {
            let file_path = walk_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            if let Some(cutoff_time) = cutoff
                && let Ok(meta) = fs::metadata(file_path)
                && let Ok(mtime) = meta.modified()
                && mtime < cutoff_time
            {
                continue;
            }

            if let Some(filter) = project_filter {
                match Self::session_cwd(file_path) {
                    Ok(Some(cwd)) if codex_project_filter_matches(&cwd, filter) => {}
                    _ => continue,
                }
            }

            sessions.push(file_path.to_path_buf());
        }

        Ok(sessions)
    }

    fn session_cwd(path: &Path) -> Result<Option<String>> {
        let file =
            fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            if !line.contains("\"session_meta\"") {
                continue;
            }

            let entry: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if entry.get("type").and_then(|t| t.as_str()) == Some("session_meta") {
                return Ok(entry
                    .pointer("/payload/cwd")
                    .and_then(|c| c.as_str())
                    .map(ToOwned::to_owned));
            }
        }

        Ok(None)
    }

    fn parse_exec_command(arguments: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(arguments).ok()?;
        value
            .get("cmd")
            .or_else(|| value.get("command"))
            .and_then(|cmd| cmd.as_str())
            .map(ToOwned::to_owned)
    }

    fn parse_exec_output(raw_output: &str) -> (usize, String, bool) {
        let content = raw_output
            .split_once("\nOutput:\n")
            .map(|(_, tail)| tail)
            .unwrap_or(raw_output);
        let output_len = content.len();
        let output_preview: String = content.chars().take(1000).collect();

        let exit_code = raw_output.lines().find_map(|line| {
            line.strip_prefix("Process exited with code ")
                .and_then(|code| code.split_whitespace().next())
                .and_then(|code| code.parse::<i32>().ok())
        });

        let is_error = exit_code.is_some_and(|code| code != 0);
        (output_len, output_preview, is_error)
    }

    fn extract_commands_from_jsonl(path: &Path) -> Result<Vec<ExtractedCommand>> {
        let file =
            fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = BufReader::new(file);

        let mut pending_tool_uses: Vec<(String, String, usize)> = Vec::new();
        let mut tool_results: HashMap<String, (usize, String, bool)> = HashMap::new();
        let mut commands = Vec::new();
        let mut sequence_counter = 0;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            if !line.contains("\"function_call\"") && !line.contains("\"function_call_output\"") {
                continue;
            }

            let entry: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if entry.get("type").and_then(|t| t.as_str()) != Some("response_item") {
                continue;
            }

            match entry.pointer("/payload/type").and_then(|t| t.as_str()) {
                Some("function_call") => {
                    if entry.pointer("/payload/name").and_then(|n| n.as_str())
                        == Some("exec_command")
                        && let (Some(id), Some(args)) = (
                            entry.pointer("/payload/call_id").and_then(|i| i.as_str()),
                            entry.pointer("/payload/arguments").and_then(|a| a.as_str()),
                        )
                        && let Some(command) = Self::parse_exec_command(args)
                    {
                        pending_tool_uses.push((id.to_string(), command, sequence_counter));
                        sequence_counter += 1;
                    }
                }
                Some("function_call_output") => {
                    if let Some(id) = entry.pointer("/payload/call_id").and_then(|i| i.as_str()) {
                        let output = entry
                            .pointer("/payload/output")
                            .and_then(|o| o.as_str())
                            .unwrap_or("");
                        let (output_len, output_content, is_error) =
                            Self::parse_exec_output(output);

                        tool_results.insert(id.to_string(), (output_len, output_content, is_error));
                    }
                }
                _ => {}
            }
        }

        for (tool_id, command, sequence_index) in pending_tool_uses {
            let (output_len, output_content, is_error) = tool_results
                .get(&tool_id)
                .map(|(len, content, err)| (Some(*len), Some(content.clone()), *err))
                .unwrap_or((None, None, false));

            commands.push(ExtractedCommand {
                command,
                output_len,
                output_content,
                is_error,
                sequence_index,
            });
        }

        Ok(commands)
    }
}

impl SessionProvider for CodexProvider {
    fn discover_sessions(
        &self,
        project_filter: Option<&str>,
        since_days: Option<u64>,
    ) -> Result<Vec<PathBuf>> {
        let sessions_dir = Self::sessions_dir()?;
        Self::discover_sessions_in(&sessions_dir, project_filter, since_days)
    }

    fn extract_commands(&self, path: &Path) -> Result<Vec<ExtractedCommand>> {
        Self::extract_commands_from_jsonl(path)
    }
}
