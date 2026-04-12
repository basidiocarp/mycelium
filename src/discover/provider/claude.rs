use super::shared::cutoff_time;
use super::{ExtractedCommand, SessionProvider};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Session provider that reads Claude Code JSONL files from `~/.claude/projects/`.
pub struct ClaudeProvider;

impl ClaudeProvider {
    /// Get the base directory for Claude Code projects.
    fn projects_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let dir = home.join(".claude").join("projects");
        if !dir.exists() {
            anyhow::bail!(
                "Claude Code projects directory not found: {}\nMake sure Claude Code has been used at least once.",
                dir.display()
            );
        }
        Ok(dir)
    }

    /// Encode a filesystem path to Claude Code's directory name format.
    /// `/Users/foo/bar` → `-Users-foo-bar`
    pub fn encode_project_path(path: &str) -> String {
        path.replace('/', "-")
    }

    /// Whether Claude Code history is available.
    pub fn history_root_exists() -> bool {
        dirs::home_dir()
            .map(|home| home.join(".claude").join("projects").exists())
            .unwrap_or(false)
    }
}

impl SessionProvider for ClaudeProvider {
    fn discover_sessions(
        &self,
        project_filter: Option<&str>,
        since_days: Option<u64>,
    ) -> Result<Vec<PathBuf>> {
        let projects_dir = Self::projects_dir()?;
        let cutoff = cutoff_time(since_days);
        let mut sessions = Vec::new();

        let entries = fs::read_dir(&projects_dir)
            .with_context(|| format!("failed to read {}", projects_dir.display()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(filter) = project_filter {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !dir_name.contains(filter) {
                    continue;
                }
            }

            for walk_entry in WalkBuilder::new(&path)
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

                sessions.push(file_path.to_path_buf());
            }
        }

        Ok(sessions)
    }

    fn extract_commands(&self, path: &Path) -> Result<Vec<ExtractedCommand>> {
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

            if !line.contains("\"Bash\"") && !line.contains("\"tool_result\"") {
                continue;
            }

            let entry: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match entry_type {
                "assistant" => {
                    if let Some(content) =
                        entry.pointer("/message/content").and_then(|c| c.as_array())
                    {
                        for block in content {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                                && block.get("name").and_then(|n| n.as_str()) == Some("Bash")
                                && let (Some(id), Some(cmd)) = (
                                    block.get("id").and_then(|i| i.as_str()),
                                    block.pointer("/input/command").and_then(|c| c.as_str()),
                                )
                            {
                                pending_tool_uses.push((
                                    id.to_string(),
                                    cmd.to_string(),
                                    sequence_counter,
                                ));
                                sequence_counter += 1;
                            }
                        }
                    }
                }
                "user" => {
                    if let Some(content) =
                        entry.pointer("/message/content").and_then(|c| c.as_array())
                    {
                        for block in content {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                                && let Some(id) = block.get("tool_use_id").and_then(|i| i.as_str())
                            {
                                let content =
                                    block.get("content").and_then(|c| c.as_str()).unwrap_or("");
                                let output_len = content.len();
                                let is_error = block
                                    .get("is_error")
                                    .and_then(|e| e.as_bool())
                                    .unwrap_or(false);
                                let content_preview: String = content.chars().take(1000).collect();

                                tool_results.insert(
                                    id.to_string(),
                                    (output_len, content_preview, is_error),
                                );
                            }
                        }
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
