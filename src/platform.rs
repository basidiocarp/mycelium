use std::path::PathBuf;
use std::process::Command;

pub fn mycelium_config_dir() -> Option<PathBuf> {
    Some(spore::paths::config_dir("mycelium"))
}

pub fn mycelium_data_dir() -> Option<PathBuf> {
    Some(spore::paths::data_dir("mycelium"))
}

pub fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude"))
}

pub fn claude_settings_path() -> Option<PathBuf> {
    claude_dir().map(|dir| dir.join("settings.json"))
}

pub fn claude_hooks_dir() -> Option<PathBuf> {
    claude_dir().map(|dir| dir.join("hooks"))
}

pub fn command_path(command: &str) -> Option<PathBuf> {
    which::which(command).ok()
}

pub fn command_on_path(command: &str) -> bool {
    command_path(command).is_some()
}

fn preferred_shell_program() -> String {
    if cfg!(target_os = "windows") {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

pub fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new(preferred_shell_program());
    if cfg!(target_os = "windows") {
        cmd.args(["/C", command]);
    } else {
        cmd.args(["-c", command]);
    }
    cmd
}

pub fn invoke_shell_command(command: &str) -> Command {
    let mut cmd = Command::new(preferred_shell_program());
    if cfg!(target_os = "windows") {
        cmd.args(["/C", command]);
    } else {
        cmd.args(["-l", "-c", command]);
    }
    cmd
}

pub fn render_shell_command(args: &[String]) -> String {
    args.iter()
        .map(|arg| shell_escape_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape_arg(arg: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        escape_windows_arg(arg)
    }
    #[cfg(not(target_os = "windows"))]
    {
        escape_posix_arg(arg)
    }
}

#[cfg(not(target_os = "windows"))]
fn escape_posix_arg(arg: &str) -> String {
    if !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | '+' | '=')
        })
    {
        return arg.to_string();
    }

    format!("'{}'", arg.replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "windows")]
fn escape_windows_arg(arg: &str) -> String {
    if !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | '+' | '=' | '\\')
        })
    {
        return arg.to_string();
    }

    format!("\"{}\"", arg.replace('"', "\\\""))
}

pub fn split_env_paths(value: &str) -> Vec<PathBuf> {
    std::env::split_paths(value).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mycelium_config_dir_shape() {
        if let Some(path) = mycelium_config_dir() {
            assert!(path.ends_with(PathBuf::from("mycelium")));
        }
    }

    #[test]
    fn test_mycelium_data_dir_shape() {
        if let Some(path) = mycelium_data_dir() {
            assert!(path.ends_with(PathBuf::from("mycelium")));
        }
    }

    #[test]
    fn test_claude_paths_shape() {
        if let Some(path) = claude_settings_path() {
            assert!(path.ends_with(PathBuf::from(".claude").join("settings.json")));
        }
        if let Some(path) = claude_hooks_dir() {
            assert!(path.ends_with(PathBuf::from(".claude").join("hooks")));
        }
    }

    #[test]
    fn test_split_env_paths_uses_platform_separator() {
        let joined = std::env::join_paths([PathBuf::from("one"), PathBuf::from("two")])
            .expect("valid path list");
        let joined = joined.to_string_lossy().to_string();
        let parts = split_env_paths(&joined);
        assert_eq!(parts, vec![PathBuf::from("one"), PathBuf::from("two")]);
    }

    #[cfg(unix)]
    #[test]
    fn test_command_path_finds_executable_on_path() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("tempdir");
        let bin_path = temp.path().join("mycelium-platform-test-bin");
        std::fs::write(&bin_path, "#!/bin/sh\nexit 0\n").expect("write bin");
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).expect("chmod");

        let original_path = std::env::var_os("PATH");
        let prefixed = match original_path.as_ref() {
            Some(existing) => std::env::join_paths(
                std::iter::once(temp.path().to_path_buf()).chain(std::env::split_paths(existing)),
            )
            .expect("joined path"),
            None => std::env::join_paths([temp.path().to_path_buf()]).expect("joined path"),
        };

        unsafe {
            std::env::set_var("PATH", &prefixed);
        }

        let resolved = command_path("mycelium-platform-test-bin").expect("command path");
        assert_eq!(resolved, bin_path);
        assert!(command_on_path("mycelium-platform-test-bin"));

        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }
    }

    #[test]
    fn test_render_shell_command_keeps_simple_arguments_unquoted() {
        let args = vec!["git".to_string(), "status".to_string()];
        assert_eq!(render_shell_command(&args), "git status");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_render_shell_command_quotes_arguments_with_spaces() {
        let args = vec!["rg".to_string(), "foo bar".to_string(), "src".to_string()];
        assert_eq!(render_shell_command(&args), "rg 'foo bar' src");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_render_shell_command_escapes_single_quotes() {
        let args = vec!["printf".to_string(), "it's".to_string()];
        assert_eq!(render_shell_command(&args), "printf 'it'\"'\"'s'");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_render_shell_command_quotes_windows_arguments() {
        let args = vec!["rg".to_string(), "foo bar".to_string(), "src".to_string()];
        assert_eq!(render_shell_command(&args), "rg \"foo bar\" src");
    }
}
