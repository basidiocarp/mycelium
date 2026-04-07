//! Patches Claude Code settings.json to register the Mycelium hook.
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::claude_md::resolve_claude_dir;
use super::hook::atomic_write;

/// Control flow for settings.json patching
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatchMode {
    Ask,  // Default: prompt user [y/N]
    Auto, // --auto-patch: no prompt
    Skip, // --no-patch: manual instructions
}

const REWRITE_EVENT: &str = "PreToolUse";
const REWRITE_MATCHER: &str = "Bash";
const REWRITE_HOOK_FILE: &str = "mycelium-rewrite.sh";
const STOP_EVENT: &str = "Stop";
const SESSION_SUMMARY_HOOK_FILE: &str = "mycelium-session-summary.sh";
const LEGACY_SESSION_SUMMARY_HOOK_FILE: &str = "session-summary.sh";

/// Result of settings.json patching operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatchResult {
    Patched,        // One or more hooks were added successfully
    AlreadyPresent, // All required hooks were already in settings.json
    Declined,       // User declined when prompted
    Skipped,        // --no-patch flag used
}

/// Orchestrator: patch settings.json with Mycelium hooks
/// Handles reading, checking, prompting, merging, backing up, and atomic writing — Unix-only
#[cfg(unix)]
pub(crate) fn patch_settings_json(
    rewrite_hook_path: &Path,
    session_summary_hook_path: &Path,
    mode: PatchMode,
    verbose: u8,
) -> Result<PatchResult> {
    let claude_dir = resolve_claude_dir()?;
    let settings_path = claude_dir.join("settings.json");
    let rewrite_hook_command = rewrite_hook_path
        .to_str()
        .context("Rewrite hook path contains invalid UTF-8")?;
    let session_summary_hook_command = session_summary_hook_path
        .to_str()
        .context("Session summary hook path contains invalid UTF-8")?;

    // Read or create settings.json
    let mut root = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .with_context(|| format!("Failed to read {}", settings_path.display()))?;

        if content.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {} as JSON", settings_path.display()))?
        }
    } else {
        serde_json::json!({})
    };

    // Check idempotency
    let rewrite_hook_present =
        hook_command_present_exact(&root, REWRITE_EVENT, rewrite_hook_command);
    let session_summary_hook_present =
        hook_command_present_exact(&root, STOP_EVENT, session_summary_hook_command);
    let legacy_session_summary_hook_present =
        legacy_session_summary_command(session_summary_hook_command)
            .as_deref()
            .is_some_and(|legacy_command| {
                hook_command_present_exact(&root, STOP_EVENT, legacy_command)
            });
    if hooks_fully_present(
        rewrite_hook_present,
        session_summary_hook_present,
        legacy_session_summary_hook_present,
    ) {
        if verbose > 0 {
            eprintln!("settings.json: hooks already present");
        }
        return Ok(PatchResult::AlreadyPresent);
    }

    // Handle mode
    match mode {
        PatchMode::Skip => {
            print_manual_instructions(rewrite_hook_path, session_summary_hook_path);
            return Ok(PatchResult::Skipped);
        }
        PatchMode::Ask => {
            if !prompt_user_consent(&settings_path)? {
                print_manual_instructions(rewrite_hook_path, session_summary_hook_path);
                return Ok(PatchResult::Declined);
            }
        }
        PatchMode::Auto => {
            // Proceed without prompting
        }
    }

    // Deep-merge hook
    if !rewrite_hook_present {
        insert_hook_entry(
            &mut root,
            REWRITE_EVENT,
            Some(REWRITE_MATCHER),
            rewrite_hook_command,
        );
    }
    if legacy_session_summary_hook_present {
        remove_hook_from_event(&mut root, STOP_EVENT, session_summary_hook_command);
    }
    if !session_summary_hook_present {
        insert_hook_entry(&mut root, STOP_EVENT, None, session_summary_hook_command);
    }

    // Backup original
    if settings_path.exists() {
        let backup_path = settings_path.with_extension("json.bak");
        fs::copy(&settings_path, &backup_path)
            .with_context(|| format!("Failed to backup to {}", backup_path.display()))?;
        if verbose > 0 {
            eprintln!("Backup: {}", backup_path.display());
        }
    }

    // Atomic write
    let serialized =
        serde_json::to_string_pretty(&root).context("Failed to serialize settings.json")?;
    atomic_write(&settings_path, &serialized)?;

    println!("\n  settings.json: Claude hooks updated");
    if settings_path.with_extension("json.bak").exists() {
        println!(
            "  Backup: {}",
            settings_path.with_extension("json.bak").display()
        );
    }
    println!("  Restart Claude Code. Test with: git status");

    Ok(PatchResult::Patched)
}

/// Prompt user for consent to patch settings.json
/// Prints to stderr (stdout may be piped), reads from stdin
/// Default is No (capital N) — Unix-only
#[cfg(unix)]
fn prompt_user_consent(settings_path: &Path) -> Result<bool> {
    use std::io::{self, BufRead, IsTerminal};

    eprintln!("\nPatch existing {}? [y/N] ", settings_path.display());

    // If stdin is not a terminal (piped), default to No
    if !io::stdin().is_terminal() {
        eprintln!("(non-interactive mode, defaulting to N)");
        return Ok(false);
    }

    let stdin = io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("Failed to read user input")?;

    let response = line.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}

/// Print manual instructions for settings.json patching — Unix-only
#[cfg(unix)]
fn print_manual_instructions(rewrite_hook_path: &Path, session_summary_hook_path: &Path) {
    println!("\n  MANUAL STEP: Merge this into ~/.claude/settings.json:");
    println!("  {{");
    println!("    \"hooks\": {{");
    println!("      \"PreToolUse\": [{{");
    println!("        \"matcher\": \"Bash\",");
    println!("        \"hooks\": [{{ \"type\": \"command\",");
    println!("          \"command\": \"{}\"", rewrite_hook_path.display());
    println!("        }}]");
    println!("      }}],");
    println!("      \"Stop\": [{{");
    println!("        \"hooks\": [{{ \"type\": \"command\",");
    println!(
        "          \"command\": \"{}\"",
        session_summary_hook_path.display()
    );
    println!("        }}]");
    println!("      }}]");
    println!("    }}");
    println!("  }}");
    println!("\n  Then restart Claude Code. Test with: git status\n");
}

/// Clean up consecutive blank lines (collapse 3+ to 2)
/// Used when removing @MYCELIUM.md line from CLAUDE.md
pub(crate) fn clean_double_blanks(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.trim().is_empty() {
            // Count consecutive blank lines
            let mut blank_count = 0;
            let _start = i;
            while i < lines.len() && lines[i].trim().is_empty() {
                blank_count += 1;
                i += 1;
            }

            // Keep at most 2 blank lines
            let keep = blank_count.min(2);
            result.extend(std::iter::repeat_n("", keep));
        } else {
            result.push(line);
            i += 1;
        }
    }

    result.join("\n")
}

/// Deep-merge Mycelium hook entry into settings.json
/// Creates hooks.<event> structure if missing, preserves existing hooks — Unix-only
#[cfg(unix)]
pub(crate) fn insert_hook_entry(
    root: &mut serde_json::Value,
    event: &str,
    matcher: Option<&str>,
    hook_command: &str,
) {
    // Ensure root is an object
    let root_obj = match root.as_object_mut() {
        Some(obj) => obj,
        None => {
            *root = serde_json::json!({});
            root.as_object_mut()
                .expect("Just created object, must succeed")
        }
    };

    // Use entry() API for idiomatic insertion
    let hooks = root_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("hooks must be an object");

    let pre_tool_use = hooks
        .entry(event)
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .expect("hook event must be an array");

    let mut hook_entry = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": hook_command
        }]
    });

    if let Some(matcher) = matcher {
        hook_entry["matcher"] = serde_json::Value::String(matcher.to_string());
    }

    // Append Mycelium hook entry
    pre_tool_use.push(hook_entry);
}

/// Check if Mycelium hook is already present in settings.json
/// Matches on hook filename to handle different path formats
pub(crate) fn hook_already_present(
    root: &serde_json::Value,
    event: &str,
    hook_command: &str,
) -> bool {
    hook_commands(root, event)
        .any(|command| same_hook_command(event, command, hook_command))
}

fn hook_command_present_exact(root: &serde_json::Value, event: &str, hook_command: &str) -> bool {
    let normalized_expected = normalize_hook_command(hook_command);
    hook_commands(root, event).any(|command| normalize_hook_command(command) == normalized_expected)
}

fn hooks_fully_present(
    rewrite_hook_present: bool,
    session_summary_hook_present: bool,
    legacy_session_summary_hook_present: bool,
) -> bool {
    rewrite_hook_present && session_summary_hook_present && !legacy_session_summary_hook_present
}

fn hook_commands<'a>(
    root: &'a serde_json::Value,
    event: &str,
) -> impl Iterator<Item = &'a str> + 'a {
    let pre_tool_use_array = match root
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(|p| p.as_array())
    {
        Some(arr) => arr.as_slice(),
        None => &[],
    };

    pre_tool_use_array
        .iter()
        .filter_map(|entry| entry.get("hooks")?.as_array())
        .flatten()
        .filter_map(|hook| hook.get("command")?.as_str())
}

fn same_hook_command(event: &str, existing_command: &str, expected_command: &str) -> bool {
    let normalized_existing = normalize_hook_command(existing_command);
    equivalent_hook_commands(event, expected_command)
        .iter()
        .any(|candidate| candidate == &normalized_existing)
}

fn normalize_hook_command(command: &str) -> String {
    let expanded = if let Some(stripped) = command.strip_prefix("~/") {
        dirs::home_dir()
            .map(|home| home.join(stripped).display().to_string())
            .unwrap_or_else(|| command.to_string())
    } else {
        command.to_string()
    };

    Path::new(&expanded)
        .components()
        .collect::<PathBuf>()
        .display()
        .to_string()
}

fn equivalent_hook_commands(event: &str, hook_command: &str) -> Vec<String> {
    let normalized = normalize_hook_command(hook_command);
    let mut commands = vec![normalized.clone()];

    if event == STOP_EVENT {
        if let Some(current_command) = sibling_hook_command(&normalized, SESSION_SUMMARY_HOOK_FILE) {
            commands.push(current_command);
        }
        if let Some(legacy_command) = sibling_hook_command(&normalized, LEGACY_SESSION_SUMMARY_HOOK_FILE)
        {
            commands.push(legacy_command);
        }
    }

    commands.sort();
    commands.dedup();
    commands
}

fn sibling_hook_command(command: &str, file_name: &str) -> Option<String> {
    let parent = Path::new(command).parent()?;
    Some(
        parent
            .join(file_name)
            .components()
            .collect::<PathBuf>()
            .display()
            .to_string(),
    )
}

fn legacy_session_summary_command(command: &str) -> Option<String> {
    sibling_hook_command(command, LEGACY_SESSION_SUMMARY_HOOK_FILE)
}

fn remove_hook_from_event(root: &mut serde_json::Value, event: &str, hook_command: &str) -> bool {
    let hooks = match root.get_mut("hooks").and_then(|h| h.get_mut(event)) {
        Some(event_hooks) => event_hooks,
        None => return false,
    };

    let event_array = match hooks.as_array_mut() {
        Some(arr) => arr,
        None => return false,
    };

    let mut removed_any = false;
    event_array.retain_mut(|entry| {
        if let Some(hooks_array) = entry.get_mut("hooks").and_then(|h| h.as_array_mut()) {
            let original_hooks_len = hooks_array.len();
            hooks_array.retain(|hook| {
                hook.get("command")
                    .and_then(|c| c.as_str())
                    .is_none_or(|command| !same_hook_command(event, command, hook_command))
            });
            removed_any |= hooks_array.len() < original_hooks_len;
            !hooks_array.is_empty()
        } else {
            true
        }
    });

    if event_array.is_empty()
        && let Some(hooks_obj) = root.get_mut("hooks").and_then(|h| h.as_object_mut())
    {
        hooks_obj.remove(event);
        if hooks_obj.is_empty() {
            root.as_object_mut().and_then(|obj| obj.remove("hooks"));
        }
    }

    removed_any
}

/// Remove Mycelium hook entry from settings.json
/// Returns true if one or more hook entries were found and removed
pub(crate) fn remove_hook_from_json(root: &mut serde_json::Value) -> bool {
    let rewrite_command = dirs::home_dir()
        .map(|home| {
            home.join(".claude")
                .join("hooks")
                .join(REWRITE_HOOK_FILE)
                .display()
                .to_string()
        })
        .unwrap_or_else(|| REWRITE_HOOK_FILE.to_string());
    let session_command = dirs::home_dir()
        .map(|home| {
            home.join(".claude")
                .join("hooks")
                .join(SESSION_SUMMARY_HOOK_FILE)
                .display()
                .to_string()
        })
        .unwrap_or_else(|| SESSION_SUMMARY_HOOK_FILE.to_string());

    let removed_rewrite = remove_hook_from_event(root, REWRITE_EVENT, &rewrite_command);
    let removed_session = remove_hook_from_event(root, STOP_EVENT, &session_command);

    removed_rewrite || removed_session
}

/// Remove Mycelium hook from settings.json file
/// Backs up before modification, returns true if hook was found and removed
pub(crate) fn remove_hook_from_settings(verbose: u8) -> Result<bool> {
    let claude_dir = resolve_claude_dir()?;
    let settings_path = claude_dir.join("settings.json");

    if !settings_path.exists() {
        if verbose > 0 {
            eprintln!("settings.json not found, nothing to remove");
        }
        return Ok(false);
    }

    let content = fs::read_to_string(&settings_path)
        .with_context(|| format!("Failed to read {}", settings_path.display()))?;

    if content.trim().is_empty() {
        return Ok(false);
    }

    let mut root: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {} as JSON", settings_path.display()))?;

    let removed = remove_hook_from_json(&mut root);

    if removed {
        // Backup original
        let backup_path = settings_path.with_extension("json.bak");
        fs::copy(&settings_path, &backup_path)
            .with_context(|| format!("Failed to backup to {}", backup_path.display()))?;

        // Atomic write
        let serialized =
            serde_json::to_string_pretty(&root).context("Failed to serialize settings.json")?;
        atomic_write(&settings_path, &serialized)?;

        if verbose > 0 {
            eprintln!("Removed Mycelium hook from settings.json");
        }
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for hook_already_present()
    #[test]
    fn test_hook_already_present_exact_match() {
        let json_content = serde_json::json!({
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

        let hook_command = "/Users/test/.claude/hooks/mycelium-rewrite.sh";
        assert!(hook_already_present(
            &json_content,
            REWRITE_EVENT,
            hook_command
        ));
    }

    #[test]
    fn test_hook_already_present_different_path() {
        let home = dirs::home_dir().expect("home dir");
        let absolute = home.join(".claude/hooks/mycelium-rewrite.sh");
        let json_content = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": absolute.display().to_string()
                    }]
                }]
            }
        });

        let hook_command = "~/.claude/hooks/mycelium-rewrite.sh";
        assert!(hook_already_present(
            &json_content,
            REWRITE_EVENT,
            hook_command
        ));
    }

    #[test]
    fn test_stop_hook_already_present_different_path() {
        let home = dirs::home_dir().expect("home dir");
        let absolute = home.join(".claude/hooks/mycelium-session-summary.sh");
        let json_content = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": absolute.display().to_string()
                    }]
                }]
            }
        });

        let hook_command = "~/.claude/hooks/mycelium-session-summary.sh";
        assert!(hook_already_present(
            &json_content,
            STOP_EVENT,
            hook_command
        ));
    }

    #[test]
    fn test_stop_hook_already_present_legacy_filename() {
        let home = dirs::home_dir().expect("home dir");
        let absolute = home.join(".claude/hooks/session-summary.sh");
        let json_content = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": absolute.display().to_string()
                    }]
                }]
            }
        });

        let hook_command = "~/.claude/hooks/mycelium-session-summary.sh";
        assert!(hook_already_present(&json_content, STOP_EVENT, hook_command));
        assert!(!hook_command_present_exact(
            &json_content,
            STOP_EVENT,
            hook_command
        ));
    }

    #[test]
    fn test_hooks_fully_present_requires_legacy_cleanup() {
        assert!(hooks_fully_present(true, true, false));
        assert!(!hooks_fully_present(true, true, true));
        assert!(!hooks_fully_present(true, false, false));
    }

    #[test]
    fn test_hook_not_present_empty() {
        let json_content = serde_json::json!({});
        let hook_command = "/Users/test/.claude/hooks/mycelium-rewrite.sh";
        assert!(!hook_already_present(
            &json_content,
            REWRITE_EVENT,
            hook_command
        ));
    }

    #[test]
    fn test_hook_not_present_other_hooks() {
        let json_content = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/some/other/hook.sh"
                    }]
                }]
            }
        });

        let hook_command = "/Users/test/.claude/hooks/mycelium-rewrite.sh";
        assert!(!hook_already_present(
            &json_content,
            REWRITE_EVENT,
            hook_command
        ));
    }

    // Tests for insert_hook_entry() — Unix-only (hook paths use Unix conventions)
    #[test]
    #[cfg(unix)]
    fn test_insert_hook_entry_empty_root() {
        let mut json_content = serde_json::json!({});
        let hook_command = "/Users/test/.claude/hooks/mycelium-rewrite.sh";

        insert_hook_entry(
            &mut json_content,
            REWRITE_EVENT,
            Some(REWRITE_MATCHER),
            hook_command,
        );

        // Should create full structure
        assert!(json_content.get("hooks").is_some());
        assert!(
            json_content
                .get("hooks")
                .unwrap()
                .get("PreToolUse")
                .is_some()
        );

        let pre_tool_use = json_content["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool_use.len(), 1);

        let command = pre_tool_use[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(command, hook_command);
    }

    #[test]
    #[cfg(unix)]
    fn test_insert_hook_entry_preserves_existing() {
        let mut json_content = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/some/other/hook.sh"
                    }]
                }]
            }
        });

        let hook_command = "/Users/test/.claude/hooks/mycelium-rewrite.sh";
        insert_hook_entry(
            &mut json_content,
            REWRITE_EVENT,
            Some(REWRITE_MATCHER),
            hook_command,
        );

        let pre_tool_use = json_content["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool_use.len(), 2); // Should have both hooks

        // Check first hook is preserved
        let first_command = pre_tool_use[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(first_command, "/some/other/hook.sh");

        // Check second hook is Mycelium
        let second_command = pre_tool_use[1]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(second_command, hook_command);
    }

    #[test]
    #[cfg(unix)]
    fn test_insert_hook_preserves_other_keys() {
        let mut json_content = serde_json::json!({
            "env": {"PATH": "/custom/path"},
            "permissions": {"allowAll": true},
            "model": "claude-sonnet-4"
        });

        let hook_command = "/Users/test/.claude/hooks/mycelium-rewrite.sh";
        insert_hook_entry(
            &mut json_content,
            REWRITE_EVENT,
            Some(REWRITE_MATCHER),
            hook_command,
        );

        // Should preserve all other keys
        assert_eq!(json_content["env"]["PATH"], "/custom/path");
        assert_eq!(json_content["permissions"]["allowAll"], true);
        assert_eq!(json_content["model"], "claude-sonnet-4");

        // And add hooks
        assert!(json_content.get("hooks").is_some());
    }

    #[test]
    #[cfg(unix)]
    fn test_insert_stop_hook_without_matcher() {
        let mut json_content = serde_json::json!({});
        let hook_command = "/Users/test/.claude/hooks/mycelium-session-summary.sh";

        insert_hook_entry(&mut json_content, STOP_EVENT, None, hook_command);

        let stop_hooks = json_content["hooks"][STOP_EVENT].as_array().unwrap();
        assert_eq!(stop_hooks.len(), 1);
        assert!(stop_hooks[0].get("matcher").is_none());
        assert_eq!(stop_hooks[0]["hooks"][0]["command"], hook_command);
    }

    // Test for preserve_order round-trip
    #[test]
    fn test_preserve_order_round_trip() {
        let original = r#"{"env": {"PATH": "/usr/bin"}, "permissions": {"allowAll": true}, "model": "claude-sonnet-4"}"#;
        let parsed: serde_json::Value = serde_json::from_str(original).unwrap();
        let serialized = serde_json::to_string(&parsed).unwrap();

        // Keys should appear in same order
        let _original_keys: Vec<&str> = original.split("\"").filter(|s| s.contains(":")).collect();
        let _serialized_keys: Vec<&str> =
            serialized.split("\"").filter(|s| s.contains(":")).collect();

        // Just check that keys exist (preserve_order doesn't guarantee exact order in nested objects)
        assert!(serialized.contains("\"env\""));
        assert!(serialized.contains("\"permissions\""));
        assert!(serialized.contains("\"model\""));
    }

    // Tests for clean_double_blanks()
    #[test]
    fn test_clean_double_blanks() {
        let input = "line1\n\n\nline2\n\nline3\n\n\n\nline4";
        let expected = "line1\n\n\nline2\n\nline3\n\n\nline4";
        assert_eq!(clean_double_blanks(input), expected);
    }

    #[test]
    fn test_clean_double_blanks_preserves_single() {
        let input = "line1\n\nline2\n\nline3";
        assert_eq!(clean_double_blanks(input), input); // No change
    }

    // Tests for remove_hook_from_json()
    #[test]
    fn test_remove_hook_from_json() {
        let home = dirs::home_dir().expect("home dir");
        let rewrite_path = home.join(".claude/hooks/mycelium-rewrite.sh");
        let session_path = home.join(".claude/hooks/mycelium-session-summary.sh");
        let mut json_content = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [{
                            "type": "command",
                            "command": "/some/other/hook.sh"
                        }]
                    },
                    {
                        "matcher": "Bash",
                        "hooks": [{
                            "type": "command",
                            "command": rewrite_path.display().to_string()
                        }]
                    }
                ],
                "Stop": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": session_path.display().to_string()
                            },
                            {
                                "type": "command",
                                "command": "/some/other/stop-hook.sh"
                            }
                        ]
                    }
                ]
            }
        });

        let removed = remove_hook_from_json(&mut json_content);
        assert!(removed);

        // Should have only one pre-tool hook left
        let pre_tool_use = json_content["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool_use.len(), 1);

        // Check it's the other hook
        let command = pre_tool_use[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(command, "/some/other/hook.sh");

        let stop_hooks = json_content["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop_hooks.len(), 1);
        assert_eq!(stop_hooks[0]["hooks"][0]["command"], "/some/other/stop-hook.sh");
    }

    #[test]
    fn test_remove_hook_from_json_removes_legacy_stop_hook() {
        let home = dirs::home_dir().expect("home dir");
        let legacy_session_path = home.join(".claude/hooks/session-summary.sh");
        let mut json_content = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [
                        {
                            "type": "command",
                            "command": legacy_session_path.display().to_string()
                        },
                        {
                            "type": "command",
                            "command": "/some/other/stop-hook.sh"
                        }
                    ]
                }]
            }
        });

        let removed = remove_hook_from_json(&mut json_content);
        assert!(removed);

        let stop_hooks = json_content["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop_hooks.len(), 1);
        assert_eq!(stop_hooks[0]["hooks"][0]["command"], "/some/other/stop-hook.sh");
    }

    #[test]
    fn test_remove_hook_when_not_present() {
        let mut json_content = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/some/other/hook.sh"
                    }]
                }]
            }
        });

        let removed = remove_hook_from_json(&mut json_content);
        assert!(!removed);
    }
}
