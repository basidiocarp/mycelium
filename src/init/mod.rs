//! Bootstraps Mycelium integration by installing hooks and patching CLAUDE.md.
#[cfg_attr(not(unix), allow(dead_code))]
mod claude_md;
mod config_show;
mod hook;
pub(crate) mod host_status;
#[cfg_attr(not(unix), allow(dead_code))]
mod json_patch;
mod install;
mod onboard;

use anyhow::Result;
use std::fs;

pub use json_patch::PatchMode;
pub use config_show::show_config;

use claude_md::{
    remove_mycelium_block, resolve_claude_dir,
};
use json_patch::{clean_double_blanks, remove_hook_from_settings};
use install::{run_default_mode, run_hook_only_mode, run_claude_md_mode};

const LEGACY_SESSION_SUMMARY_HOOK_NAME: &str = "session-summary.sh";

/// Main entry point for `mycelium init`
pub fn run(
    global: bool,
    claude_md: bool,
    hook_only: bool,
    patch_mode: PatchMode,
    verbose: u8,
) -> Result<()> {
    // Mode selection
    match (claude_md, hook_only) {
        (true, _) => run_claude_md_mode(global, verbose),
        (false, true) => run_hook_only_mode(global, patch_mode, verbose),
        (false, false) => run_default_mode(global, patch_mode, verbose),
    }
}

pub fn onboard(global: bool, verbose: u8) -> Result<()> {
    onboard::run(global, verbose)
}

/// Full uninstall: remove hooks, MYCELIUM.md, @MYCELIUM.md reference, settings.json entry
pub fn uninstall(global: bool, verbose: u8) -> Result<()> {
    if !global {
        anyhow::bail!(
            "Uninstall only works with --global flag. For local projects, manually remove Mycelium from CLAUDE.md"
        );
    }

    let claude_dir = resolve_claude_dir()?;
    let mut removed = Vec::new();

    // 1. Remove hook files
    let hook_dir = claude_dir.join("hooks");
    let hook_path = hook_dir.join("mycelium-rewrite.sh");
    if hook_path.exists() {
        fs::remove_file(&hook_path)
            .with_context(|| format!("Failed to remove hook: {}", hook_path.display()))?;
        removed.push(format!("Hook: {}", hook_path.display()));
    }

    let session_summary_hook_path = hook_dir.join("mycelium-session-summary.sh");
    if session_summary_hook_path.exists() {
        fs::remove_file(&session_summary_hook_path).with_context(|| {
            format!(
                "Failed to remove session hook: {}",
                session_summary_hook_path.display()
            )
        })?;
        removed.push(format!(
            "Session hook: {}",
            session_summary_hook_path.display()
        ));
    }

    // 1b. Remove integrity hash file
    if crate::integrity::remove_hash(&hook_path)? {
        removed.push("Integrity hash: removed".to_string());
    }
    if crate::integrity::remove_hash(&session_summary_hook_path)? {
        removed.push("Session hook integrity hash: removed".to_string());
    }

    let legacy_session_summary_hook_path = hook_dir.join(LEGACY_SESSION_SUMMARY_HOOK_NAME);
    if legacy_session_summary_hook_path.exists() {
        fs::remove_file(&legacy_session_summary_hook_path).with_context(|| {
            format!(
                "Failed to remove legacy session hook: {}",
                legacy_session_summary_hook_path.display()
            )
        })?;
        removed.push(format!(
            "Legacy session hook: {}",
            legacy_session_summary_hook_path.display()
        ));
    }
    if crate::integrity::remove_hash(&legacy_session_summary_hook_path)? {
        removed.push("Legacy session hook integrity hash: removed".to_string());
    }

    // 2. Remove MYCELIUM.md
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    if mycelium_md_path.exists() {
        fs::remove_file(&mycelium_md_path).with_context(|| {
            format!(
                "Failed to remove MYCELIUM.md: {}",
                mycelium_md_path.display()
            )
        })?;
        removed.push(format!("MYCELIUM.md: {}", mycelium_md_path.display()));
    }

    // 3. Remove @MYCELIUM.md reference or legacy instructions block from CLAUDE.md
    let claude_md_path = claude_dir.join("CLAUDE.md");
    if claude_md_path.exists() {
        let content = fs::read_to_string(&claude_md_path)
            .with_context(|| format!("Failed to read CLAUDE.md: {}", claude_md_path.display()))?;

        let (without_legacy_block, removed_legacy_block) =
            if content.contains("<!-- mycelium-instructions") {
                remove_mycelium_block(&content)
            } else {
                (content.clone(), false)
            };

        let mut removed_anything = false;
        let mut new_content = without_legacy_block;

        if content.contains("@MYCELIUM.md") {
            new_content = new_content
                .lines()
                .filter(|line| !line.trim().starts_with("@MYCELIUM.md"))
                .collect::<Vec<_>>()
                .join("\n");
            removed_anything = true;
        }

        if removed_legacy_block {
            removed_anything = true;
        }

        if removed_anything {
            let cleaned = clean_double_blanks(&new_content);
            fs::write(&claude_md_path, cleaned).with_context(|| {
                format!("Failed to write CLAUDE.md: {}", claude_md_path.display())
            })?;
            if content.contains("@MYCELIUM.md") {
                removed.push("CLAUDE.md: removed @MYCELIUM.md reference".to_string());
            }
            if removed_legacy_block {
                removed.push("CLAUDE.md: removed legacy Mycelium instructions".to_string());
            }
        }
    }

    // 4. Remove hook entry from settings.json
    if remove_hook_from_settings(verbose)? {
        removed.push("settings.json: removed Mycelium hook entry".to_string());
    }

    // Report results
    if removed.is_empty() {
        println!("Mycelium was not installed (nothing to remove)");
    } else {
        println!("Mycelium uninstalled:");
        for item in removed {
            println!("  - {}", item);
        }
        println!("\nRestart Claude Code to apply changes.");
    }

    Ok(())
}


// Need with_context for uninstall
use anyhow::Context;

pub mod context;
