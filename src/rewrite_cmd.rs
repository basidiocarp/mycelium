//! Rewrites shell commands to their Mycelium equivalents for transparent hook integration.
use crate::discover::registry;

/// Run the `mycelium rewrite` command.
///
/// Prints the Mycelium-rewritten command to stdout and exits 0.
/// Exits 1 (without output) if the command has no Mycelium equivalent.
///
/// Used by shell hooks to rewrite commands transparently:
/// ```bash
/// REWRITTEN=$(mycelium rewrite "$CMD") || exit 0
/// [ "$CMD" = "$REWRITTEN" ] && exit 0  # already Mycelium, skip
/// ```
pub fn run(cmd: &str) -> anyhow::Result<()> {
    let excluded = crate::config::Config::load()
        .map(|c| c.hooks.exclude_commands)
        .unwrap_or_default();

    match registry::rewrite_command(cmd, &excluded) {
        Some(rewritten) => {
            print!("{}", rewritten);
            Ok(())
        }
        None => {
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_supported_command_succeeds() {
        assert!(registry::rewrite_command("git status", &[]).is_some());
    }

    #[test]
    fn test_run_unsupported_returns_none() {
        assert!(registry::rewrite_command("ansible-playbook site.yml", &[]).is_none());
    }

    #[test]
    fn test_run_already_mycelium_returns_some() {
        assert_eq!(
            registry::rewrite_command("mycelium git status", &[]),
            Some("mycelium git status".into())
        );
    }
}
