//! Configuration display and verification for Mycelium setup.

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::init::claude_md::resolve_claude_dir;
#[cfg(unix)]
use crate::init::hook::extract_hook_version;
use crate::init::json_patch::hook_already_present;

/// Show current mycelium configuration
pub fn show_config() -> Result<()> {
    let claude_capabilities = crate::init::host_status::claude_code_capabilities();
    let claude_dir = resolve_claude_dir()?;
    let hook_path = claude_dir.join("hooks").join("mycelium-rewrite.sh");
    let session_summary_hook_path = claude_dir.join("hooks").join("mycelium-session-summary.sh");
    let mycelium_md_path = claude_dir.join("MYCELIUM.md");
    let global_claude_md = claude_dir.join("CLAUDE.md");
    let local_claude_md = PathBuf::from("CLAUDE.md");

    println!("mycelium Configuration:\n");
    println!("Host capabilities:");
    println!(
        "  Claude hook adapter: {}",
        claude_capabilities.hook_adapter.detail
    );
    println!(
        "  Claude settings patch: {}",
        claude_capabilities.settings_patch.detail
    );
    println!(
        "  Claude global slim setup: {}",
        claude_capabilities.slim_global_setup.detail
    );
    println!(
        "  Claude CLAUDE.md mode: {}",
        claude_capabilities.legacy_claude_md.detail
    );
    println!();

    // Check rewrite hook
    if !claude_capabilities.hook_adapter.supported {
        println!("- Hook: unsupported on this platform");
    } else if hook_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&hook_path)?;
            let perms = metadata.permissions();
            let is_executable = perms.mode() & 0o111 != 0;

            let hook_content = fs::read_to_string(&hook_path)?;
            let has_guards = hook_content.contains("_resolve_command()")
                && hook_content.contains("MYCELIUM_BIN=")
                && hook_content.contains("JQ_BIN=")
                && !hook_content.contains("__MYCELIUM_VERSION__")
                && !hook_content.contains("__MYCELIUM_BIN__")
                && !hook_content.contains("__JQ_BIN__");
            let is_thin_delegator = hook_content.contains("mycelium rewrite");
            let hook_version = crate::hook_check::parse_hook_version(&hook_content);
            let installed_version = extract_hook_version(&hook_content);
            let current_version = crate::init::hook::current_install_version();

            if !is_executable {
                println!(
                    "[!] Hook: {} (NOT executable - run: chmod +x)",
                    hook_path.display()
                );
            } else if !is_thin_delegator {
                println!(
                    "[!] Hook: {} (outdated — inline logic, not thin delegator)",
                    hook_path.display()
                );
                println!(
                    "   → Run `mycelium init -g` to upgrade to the current thin delegator hook"
                );
            } else if is_executable && has_guards {
                println!(
                    "ok Hook: {} (thin delegator, version {})",
                    hook_path.display(),
                    hook_version
                );
                match installed_version.as_deref() {
                    Some(version) if version == current_version => {
                        println!("ok Hook version: {} (current)", version);
                    }
                    Some(version)
                        if crate::init::hook::version_is_stale(version, current_version) =>
                    {
                        println!(
                            "[!] Hook version: {} (stale vs current {}; run `mycelium init -g`)",
                            version, current_version
                        );
                    }
                    Some(version) => {
                        println!(
                            "[!] Hook version: {} (mismatch with current {}; run `mycelium init -g`)",
                            version, current_version
                        );
                    }
                    None => {
                        println!(
                            "[!] Hook version: unknown (run `mycelium init -g` to stamp the current version)"
                        );
                    }
                }
                let mycelium_embedded =
                    crate::init::hook::extract_quoted_assignment(&hook_content, "MYCELIUM_BIN")
                        .unwrap_or_default();
                let jq_embedded =
                    crate::init::hook::extract_quoted_assignment(&hook_content, "JQ_BIN")
                        .unwrap_or_default();
                let mycelium_on_path = crate::platform::command_on_path("mycelium");
                let jq_on_path = crate::platform::command_on_path("jq");

                if mycelium_embedded.is_empty() {
                    println!(
                        "[!] Hook dependency: mycelium was not embedded at install time; PATH fallback is required"
                    );
                    println!("    Repair: mycelium init -g");
                } else {
                    let path = PathBuf::from(&mycelium_embedded);
                    if !path.exists() {
                        println!(
                            "[!] Hook dependency: embedded mycelium path missing: {}",
                            path.display()
                        );
                        println!("    Repair: mycelium init -g");
                    }
                }

                if jq_embedded.is_empty() {
                    println!(
                        "[!] Hook dependency: jq was not embedded at install time; PATH fallback is required"
                    );
                    println!("    Repair: install jq, then run `mycelium init -g`");
                } else {
                    let path = PathBuf::from(&jq_embedded);
                    if !path.exists() {
                        println!(
                            "[!] Hook dependency: embedded jq path missing: {}",
                            path.display()
                        );
                        println!("    Repair: install jq, then run `mycelium init -g`");
                    }
                }

                let mut missing_path = Vec::new();
                if !mycelium_on_path {
                    missing_path.push("mycelium");
                }
                if !jq_on_path {
                    missing_path.push("jq");
                }
                if !missing_path.is_empty() {
                    println!(
                        "[!] Hook PATH: current PATH does not expose {}",
                        missing_path.join(" or ")
                    );
                    if !jq_on_path {
                        println!(
                            "    jq is missing from PATH; the hook will use an embedded path when available, otherwise it will skip rewrites."
                        );
                    }
                    if !mycelium_on_path {
                        println!(
                            "    mycelium is missing from PATH; the hook will use an embedded path when available, otherwise it will skip rewrites."
                        );
                    }
                }
            } else {
                println!("[!] Hook: {} (no guards - outdated)", hook_path.display());
                println!("   → Run `mycelium init -g` to refresh the guarded hook");
            }
        }

        #[cfg(not(unix))]
        {
            println!("ok Hook: {} (exists)", hook_path.display());
        }
    } else {
        println!("- Hook: not found");
    }

    // Check session summary hook
    if !claude_capabilities.hook_adapter.supported {
        println!("- Session hook: unsupported on this platform");
    } else if session_summary_hook_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&session_summary_hook_path)?;
            if metadata.permissions().mode() & 0o111 != 0 {
                println!("ok Session hook: {}", session_summary_hook_path.display());
            } else {
                println!(
                    "[!] Session hook: {} (NOT executable - run: chmod +x)",
                    session_summary_hook_path.display()
                );
            }
        }

        #[cfg(not(unix))]
        {
            println!(
                "ok Session hook: {} (exists)",
                session_summary_hook_path.display()
            );
        }
    } else {
        println!("- Session hook: not found");
    }

    if session_summary_hook_path.exists() {
        match crate::integrity::verify_hook_at(&session_summary_hook_path) {
            Ok(crate::integrity::IntegrityStatus::Verified) => {
                println!("ok Session hook integrity: verified");
            }
            Ok(crate::integrity::IntegrityStatus::Tampered { .. }) => {
                println!("[!] Session hook integrity: FAILED");
                println!("    Run: mycelium init -g to restore the Stop hook");
            }
            Ok(crate::integrity::IntegrityStatus::NoBaseline) => {
                println!("[!] Session hook integrity: no baseline hash");
                println!("    Run: mycelium init -g to record a Stop hook baseline");
            }
            Ok(crate::integrity::IntegrityStatus::NotInstalled)
            | Ok(crate::integrity::IntegrityStatus::OrphanedHash) => {}
            Err(e) => {
                println!("[!] Session hook integrity: error ({e})");
            }
        }
    }

    // Check MYCELIUM.md
    if !claude_capabilities.slim_global_setup.supported {
        println!("- MYCELIUM.md: not used by the docs-only setup on this platform");
    } else if mycelium_md_path.exists() {
        println!("ok MYCELIUM.md: {} (slim mode)", mycelium_md_path.display());
    } else {
        println!("- MYCELIUM.md: not found");
    }

    // Check hook integrity
    if claude_capabilities.hook_adapter.supported {
        match crate::integrity::verify_hook_at(&hook_path) {
            Ok(crate::integrity::IntegrityStatus::Verified) => {
                println!("ok Integrity: hook hash verified");
            }
            Ok(crate::integrity::IntegrityStatus::Tampered { .. }) => {
                println!(
                    "error: Integrity: hook modified outside mycelium init (run: mycelium verify)"
                );
            }
            Ok(crate::integrity::IntegrityStatus::NoBaseline) => {
                println!("[!] Integrity: no baseline hash (run: mycelium init -g to establish)");
            }
            Ok(crate::integrity::IntegrityStatus::NotInstalled)
            | Ok(crate::integrity::IntegrityStatus::OrphanedHash) => {
                // Don't show integrity line if hook isn't installed
            }
            Err(_) => {
                println!("[!] Integrity: check failed");
            }
        }
    }

    // Check global CLAUDE.md
    if global_claude_md.exists() {
        let content = fs::read_to_string(&global_claude_md)?;
        if content.contains("@MYCELIUM.md") {
            println!("ok Global (~/.claude/CLAUDE.md): @MYCELIUM.md reference");
        } else if content.contains("<!-- mycelium-instructions") {
            if claude_capabilities.slim_global_setup.supported {
                println!(
                    "[!] Global (~/.claude/CLAUDE.md): old Mycelium block (run: mycelium init -g to migrate)"
                );
            } else {
                println!(
                    "ok Global (~/.claude/CLAUDE.md): legacy Mycelium instructions (docs-only mode)"
                );
            }
        } else {
            println!("- Global (~/.claude/CLAUDE.md): exists but mycelium not configured");
        }
    } else {
        println!("- Global (~/.claude/CLAUDE.md): not found");
    }

    // Check local CLAUDE.md
    if local_claude_md.exists() {
        let content = fs::read_to_string(&local_claude_md)?;
        if content.contains("mycelium") {
            println!("ok Local (./CLAUDE.md): mycelium enabled");
        } else {
            println!("- Local (./CLAUDE.md): exists but mycelium not configured");
        }
    } else {
        println!("- Local (./CLAUDE.md): not found");
    }

    // Check settings.json
    let settings_path = claude_dir.join("settings.json");
    if !claude_capabilities.settings_patch.supported {
        println!("- settings.json: not managed by Mycelium on this platform");
    } else if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        if !content.trim().is_empty() {
            if let Ok(root) = serde_json::from_str::<serde_json::Value>(&content) {
                let hook_command = hook_path.display().to_string();
                let session_summary_hook_command = session_summary_hook_path.display().to_string();
                let rewrite_registered = hook_already_present(&root, "PreToolUse", &hook_command);
                let session_registered =
                    hook_already_present(&root, "Stop", &session_summary_hook_command);

                if rewrite_registered && session_registered {
                    println!("ok settings.json: Mycelium hooks configured");
                } else {
                    println!("[!] settings.json: exists but Mycelium hooks are incomplete");
                    if !rewrite_registered {
                        println!("    Missing PreToolUse hook: {}", hook_path.display());
                    }
                    if !session_registered {
                        println!(
                            "    Missing Stop hook: {}",
                            session_summary_hook_path.display()
                        );
                    }
                    println!("    Run: mycelium init -g --auto-patch");
                }
            } else {
                println!("[!] settings.json: exists but invalid JSON");
            }
        } else {
            println!("- settings.json: empty");
        }
    } else {
        println!("- settings.json: not found");
    }

    println!("\nHost adapters:");
    for status in crate::init::host_status::collect_host_adapter_statuses() {
        let marker = if status.configured { "ok" } else { "-" };
        let detected = if status.detected {
            "detected"
        } else {
            "not detected"
        };
        println!(
            "{} {}: {} ({})",
            marker, status.name, status.detail, detected
        );
    }

    println!("\nUsage:");
    println!("  mycelium init              # Local CLAUDE.md instructions");
    if claude_capabilities.hook_adapter.supported {
        println!(
            "  mycelium init -g           # Claude adapter hook + MYCELIUM.md + @MYCELIUM.md + settings.json"
        );
        println!("  mycelium init -g --auto-patch    # Same as above but no prompt");
        println!("  mycelium init -g --no-patch      # Skip settings.json (manual setup)");
        println!("  mycelium init -g --hook-only     # Hook only, no MYCELIUM.md");
    } else {
        println!("  mycelium init -g --claude-md     # Docs-only global setup on this platform");
    }
    println!("  mycelium init -g --uninstall     # Remove all Mycelium artifacts");
    println!(
        "  mycelium init -g --claude-md     # Legacy: full injection into ~/.claude/CLAUDE.md"
    );
    println!(
        "  {}                 # Preferred first-time host setup / repair flow where supported",
        crate::init::host_status::operator_setup_hint()
    );

    Ok(())
}
