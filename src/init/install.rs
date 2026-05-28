//! Installation sub-flows for Claude Code adapter and related setup.

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::init::claude_md::{
    MYCELIUM_INSTRUCTIONS, MyceliumBlockUpsert, resolve_claude_dir, upsert_mycelium_block,
};
#[cfg(unix)]
use crate::init::claude_md::{MYCELIUM_SLIM, patch_claude_md};
#[cfg(unix)]
use crate::init::hook;
use crate::init::json_patch::PatchMode;
#[cfg(unix)]
use crate::init::json_patch::patch_settings_json;

use super::context;

#[cfg(unix)]
const LEGACY_SESSION_SUMMARY_HOOK_NAME: &str = "session-summary.sh";

#[cfg(unix)]
pub(super) fn install_claude_code_adapter(patch_mode: PatchMode, verbose: u8) -> Result<()> {
    use hook::{ensure_hook_installed, ensure_session_summary_hook_installed};

    let claude_dir = resolve_claude_dir()?;
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    let claude_md_path = claude_dir.join("CLAUDE.md");

    let (_hook_dir, hook_path) = hook::prepare_hook_paths()?;
    let (_session_hook_dir, session_summary_hook_path) = hook::prepare_session_summary_hook_path()?;
    let hook_changed = ensure_hook_installed(&hook_path, verbose)?;
    let session_hook_changed =
        ensure_session_summary_hook_installed(&session_summary_hook_path, verbose)?;

    hook::write_if_changed(&mycelium_md_path, MYCELIUM_SLIM, "MYCELIUM.md", verbose)?;
    let migrated = patch_claude_md(&claude_md_path, verbose)?;

    let hook_status = if hook_changed || session_hook_changed {
        "installed/updated"
    } else {
        "already up to date"
    };
    println!("\nClaude Code adapter {} (global).\n", hook_status);
    println!("  Hook:           {}", hook_path.display());
    println!("  Session hook:   {}", session_summary_hook_path.display());
    println!(
        "  MYCELIUM.md:    {} (10 lines)",
        mycelium_md_path.display()
    );
    println!("  CLAUDE.md:      @MYCELIUM.md reference added");

    if migrated {
        println!("\n  ok Migrated: removed 137-line Mycelium block from CLAUDE.md");
        println!("              replaced with @MYCELIUM.md (10 lines)");
    }

    let patch_result =
        patch_settings_json(&hook_path, &session_summary_hook_path, patch_mode, verbose)?;
    if matches!(
        patch_result,
        crate::init::json_patch::PatchResult::Patched
            | crate::init::json_patch::PatchResult::AlreadyPresent
    ) {
        remove_legacy_session_summary_artifacts(verbose)?;
    }
    report_settings_patch_result(patch_result);
    println!();

    Ok(())
}

#[cfg(not(unix))]
pub(super) fn install_claude_code_adapter(_patch_mode: PatchMode, _verbose: u8) -> Result<()> {
    unreachable!("Claude Code hook adapter should be gated by host capabilities")
}

#[cfg(unix)]
pub(super) fn install_claude_hook_only(patch_mode: PatchMode, verbose: u8) -> Result<()> {
    use hook::{ensure_hook_installed, ensure_session_summary_hook_installed};

    let (_hook_dir, hook_path) = hook::prepare_hook_paths()?;
    let (_session_hook_dir, session_summary_hook_path) = hook::prepare_session_summary_hook_path()?;
    let hook_changed = ensure_hook_installed(&hook_path, verbose)?;
    let session_hook_changed =
        ensure_session_summary_hook_installed(&session_summary_hook_path, verbose)?;

    let hook_status = if hook_changed || session_hook_changed {
        "installed/updated"
    } else {
        "already up to date"
    };
    println!("\nClaude Code adapter {} (hook-only mode).\n", hook_status);
    println!("  Hook: {}", hook_path.display());
    println!("  Session hook: {}", session_summary_hook_path.display());
    println!(
        "  Note: No MYCELIUM.md created. Claude Code will not see Mycelium meta commands (gain, discover, proxy, invoke)."
    );

    let patch_result =
        patch_settings_json(&hook_path, &session_summary_hook_path, patch_mode, verbose)?;
    if matches!(
        patch_result,
        crate::init::json_patch::PatchResult::Patched
            | crate::init::json_patch::PatchResult::AlreadyPresent
    ) {
        remove_legacy_session_summary_artifacts(verbose)?;
    }
    report_settings_patch_result(patch_result);
    println!();

    Ok(())
}

#[cfg(not(unix))]
pub(super) fn install_claude_hook_only(_patch_mode: PatchMode, _verbose: u8) -> Result<()> {
    unreachable!("Claude Code hook adapter should be gated by host capabilities")
}

#[cfg(unix)]
fn report_settings_patch_result(patch_result: crate::init::json_patch::PatchResult) {
    use crate::init::json_patch::PatchResult;

    match patch_result {
        PatchResult::Patched => {}
        PatchResult::AlreadyPresent => {
            println!("\n  settings.json: Claude hooks already present");
            println!("  Restart Claude Code. Test with: git status");
        }
        PatchResult::Declined | PatchResult::Skipped => {}
    }
}

#[cfg(unix)]
fn remove_legacy_session_summary_artifacts(verbose: u8) -> Result<()> {
    let claude_dir = resolve_claude_dir()?;
    let legacy_hook_path = claude_dir
        .join("hooks")
        .join(LEGACY_SESSION_SUMMARY_HOOK_NAME);
    let removed_hook = if legacy_hook_path.exists() {
        fs::remove_file(&legacy_hook_path).with_context(|| {
            format!(
                "Failed to remove legacy session hook: {}",
                legacy_hook_path.display()
            )
        })?;
        true
    } else {
        false
    };
    let removed_hash = crate::integrity::remove_hash(&legacy_hook_path)?;

    if verbose > 0 && (removed_hook || removed_hash) {
        eprintln!(
            "Removed legacy session hook artifacts: {}",
            legacy_hook_path.display()
        );
    }

    Ok(())
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn report_settings_patch_result(_patch_result: crate::init::json_patch::PatchResult) {}

/// Default mode: hook + slim MYCELIUM.md + @MYCELIUM.md reference where supported.
pub(super) fn run_default_mode(global: bool, patch_mode: PatchMode, verbose: u8) -> Result<()> {
    if !global {
        // Local init: unchanged behavior (full injection into ./CLAUDE.md)
        return run_claude_md_mode(false, verbose);
    }

    let capabilities = crate::init::host_status::claude_code_capabilities();
    if !capabilities.hook_adapter.supported {
        eprintln!("[!] Claude Code hook adapter is unsupported on this platform.");
        eprintln!("    {}", capabilities.hook_adapter.detail);
        eprintln!("    Falling back to docs-only global CLAUDE.md setup.");
        return run_claude_md_mode(true, verbose);
    }

    install_claude_code_adapter(patch_mode, verbose)
}

/// Hook-only mode: just the hook, no MYCELIUM.md, where supported.
pub(super) fn run_hook_only_mode(global: bool, patch_mode: PatchMode, verbose: u8) -> Result<()> {
    if !global {
        eprintln!("[!] Warning: --hook-only only makes sense with --global");
        eprintln!("    For local projects, use default mode or --claude-md");
        return Ok(());
    }

    let capabilities = crate::init::host_status::claude_code_capabilities();
    if !capabilities.hook_adapter.supported {
        anyhow::bail!(
            "Claude Code hook adapter is unsupported on this platform. {}",
            capabilities.hook_adapter.detail
        );
    }

    install_claude_hook_only(patch_mode, verbose)
}

/// Legacy mode: full 137-line injection into CLAUDE.md
pub(super) fn run_claude_md_mode(global: bool, verbose: u8) -> Result<()> {
    let path = if global {
        resolve_claude_dir()?.join("CLAUDE.md")
    } else {
        PathBuf::from("CLAUDE.md")
    };

    if global && let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if verbose > 0 {
        eprintln!("Writing mycelium instructions to: {}", path.display());
    }

    if path.exists() {
        let existing = fs::read_to_string(&path)?;
        // upsert_mycelium_block handles all 4 cases: add, update, unchanged, malformed
        let (mut new_content, action) = upsert_mycelium_block(&existing, MYCELIUM_INSTRUCTIONS);

        match action {
            MyceliumBlockUpsert::Added => {
                // Optionally inject hyphae context after instructions
                let context_block = context::gather_context_for_init();
                if !context_block.is_empty() {
                    new_content.push('\n');
                    new_content.push_str(&context_block);
                }
                fs::write(&path, new_content)?;
                println!(
                    "ok Added mycelium instructions to existing {}",
                    path.display()
                );
            }
            MyceliumBlockUpsert::Updated => {
                // Optionally inject hyphae context after instructions
                let context_block = context::gather_context_for_init();
                if !context_block.is_empty() {
                    new_content.push('\n');
                    new_content.push_str(&context_block);
                }
                fs::write(&path, new_content)?;
                println!("ok Updated mycelium instructions in {}", path.display());
            }
            MyceliumBlockUpsert::Unchanged => {
                println!(
                    "ok {} already contains up-to-date mycelium instructions",
                    path.display()
                );
                return Ok(());
            }
            MyceliumBlockUpsert::Malformed => {
                eprintln!(
                    "[!] Warning: Found '<!-- mycelium-instructions' without closing marker in {}",
                    path.display()
                );

                if let Some((line_num, _)) = existing
                    .lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("<!-- mycelium-instructions"))
                {
                    eprintln!("    Location: line {}", line_num + 1);
                }

                eprintln!("    Action: Manually remove the incomplete block, then re-run:");
                if global {
                    eprintln!("            mycelium init -g --claude-md");
                } else {
                    eprintln!("            mycelium init --claude-md");
                }
                return Ok(());
            }
        }
    } else {
        let mut new_content = MYCELIUM_INSTRUCTIONS.to_string();
        // Optionally inject hyphae context after instructions
        let context_block = context::gather_context_for_init();
        if !context_block.is_empty() {
            new_content.push('\n');
            new_content.push_str(&context_block);
        }
        fs::write(&path, new_content)?;
        println!("ok Created {} with mycelium instructions", path.display());
    }

    if global {
        println!("   Claude Code will now use Mycelium in all sessions");
    } else {
        println!("   Claude Code will use Mycelium in this project");
    }

    Ok(())
}
