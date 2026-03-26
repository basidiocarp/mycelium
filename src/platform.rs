use std::path::PathBuf;
use std::process::Command;

pub fn mycelium_config_dir() -> Option<PathBuf> {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .map(|dir| dir.join("mycelium"))
}

pub fn mycelium_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .map(|dir| dir.join("mycelium"))
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

pub fn shell_command(command: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    }
}

pub fn invoke_shell_command(command: &str) -> Command {
    if cfg!(target_os = "windows") {
        let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd".to_string());
        let mut cmd = Command::new(shell);
        cmd.args(["/C", command]);
        cmd
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = Command::new(shell);
        cmd.args(["-lc", command]);
        cmd
    }
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
