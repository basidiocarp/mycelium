//! Persists learned CLI corrections as JSON for use by `mycelium rewrite`.
//!
//! When `mycelium learn --write-rules` runs it writes:
//!   - `.claude/rules/cli-corrections.md`   — human-readable (existing)
//!   - `.claude/rules/cli-corrections.json` — machine-readable (new)
//!
//! `mycelium rewrite` reads the JSON file from the current working directory
//! and applies matching corrections before falling through to its built-in registry.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single user-learned correction: wrong command → right command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserCorrection {
    pub wrong: String,
    pub right: String,
}

/// Default path for the machine-readable corrections file (relative to project root).
pub const CORRECTIONS_JSON: &str = ".claude/rules/cli-corrections.json";

/// Write corrections as a JSON file consumable by `mycelium rewrite`.
pub fn write_corrections_json(corrections: &[UserCorrection], path: &str) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(corrections)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Load user corrections from JSON. Returns empty Vec on missing or malformed file.
pub fn load_corrections(path: &str) -> Vec<UserCorrection> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Return the corrected command if `cmd` matches a user correction, otherwise `None`.
pub fn apply_correction(cmd: &str, corrections: &[UserCorrection]) -> Option<String> {
    let trimmed = cmd.trim();
    corrections
        .iter()
        .find(|c| c.wrong.trim() == trimmed)
        .map(|c| c.right.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_roundtrip_write_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrections.json");
        let path_str = path.to_str().unwrap();

        let corrections = vec![
            UserCorrection {
                wrong: "git commit --ammend".to_string(),
                right: "git commit --amend".to_string(),
            },
            UserCorrection {
                wrong: "gh pr edit -t".to_string(),
                right: "gh pr edit --title".to_string(),
            },
        ];

        write_corrections_json(&corrections, path_str).unwrap();
        let loaded = load_corrections(path_str);

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].wrong, "git commit --ammend");
        assert_eq!(loaded[0].right, "git commit --amend");
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let corrections = load_corrections("/tmp/does_not_exist_mycelium_test.json");
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_apply_correction_match() {
        let corrections = vec![UserCorrection {
            wrong: "git commit --ammend".to_string(),
            right: "git commit --amend".to_string(),
        }];

        assert_eq!(
            apply_correction("git commit --ammend", &corrections),
            Some("git commit --amend".to_string())
        );
        assert_eq!(apply_correction("git commit --amend", &corrections), None);
        assert_eq!(apply_correction("ls -la", &corrections), None);
    }

    #[test]
    fn test_apply_correction_trims_whitespace() {
        let corrections = vec![UserCorrection {
            wrong: "git status".to_string(),
            right: "mycelium git status".to_string(),
        }];

        assert_eq!(
            apply_correction("  git status  ", &corrections),
            Some("mycelium git status".to_string())
        );
    }
}
