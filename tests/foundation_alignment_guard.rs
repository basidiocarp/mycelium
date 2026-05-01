#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command;

#[cfg(unix)]
fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_mycelium")
}

#[cfg(unix)]
fn config_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support")
    }

    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Roaming")
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        home.join(".config")
    }
}

#[cfg(unix)]
fn write_shell_script(path: &Path, body: &str) {
    let mut file = fs::File::create(path).expect("create script");
    writeln!(file, "#!/bin/sh").expect("write shebang");
    write!(file, "{body}").expect("write body");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod script");
    }
}

#[cfg(unix)]
fn write_plugin_config(home: &Path, plugin_dir: &Path) {
    let config_root = config_dir(home).join("mycelium");
    fs::create_dir_all(&config_root).expect("create config dir");
    fs::write(
        config_root.join("config.toml"),
        format!(
            r#"[plugins]
enabled = true
directory = "{}"
"#,
            plugin_dir.display()
        ),
    )
    .expect("write config");
}

#[cfg(unix)]
fn run_mycelium(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", config_dir(home))
        .current_dir(home)
        .output()
        .expect("run mycelium")
}

#[test]
fn dispatch_source_stays_on_the_routing_boundary() {
    let dispatch = include_str!("../src/dispatch.rs");

    assert!(dispatch.contains("routes::dispatch_command"));
    assert!(dispatch.contains("plugin::find_plugin"));
    assert!(!dispatch.contains("hyphae::"));
    assert!(!dispatch.contains("rhizome::"));
}

#[test]
#[cfg(unix)]
fn plugin_filters_unknown_command_output_through_a_separate_adapter() {
    let home = tempfile::tempdir().expect("temp home");
    let plugin_dir = config_dir(home.path()).join("mycelium/plugins");
    fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    write_plugin_config(home.path(), &plugin_dir);

    let script = plugin_dir.join("echo.sh");
    write_shell_script(&script, "tr '[:lower:]' '[:upper:]'\n");

    let output = run_mycelium(home.path(), &["echo", "hello", "world"]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "HELLO WORLD\n");
    assert!(String::from_utf8_lossy(&output.stderr).trim().is_empty());
}

#[test]
#[cfg(unix)]
fn plugin_failure_replays_the_original_command_output() {
    let home = tempfile::tempdir().expect("temp home");
    let plugin_dir = config_dir(home.path()).join("mycelium/plugins");
    fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    write_plugin_config(home.path(), &plugin_dir);

    let script = plugin_dir.join("echo.sh");
    write_shell_script(
        &script,
        "printf 'PLUGIN OUTPUT\\n'\necho plugin-error >&2\nexit 3\n",
    );

    let output = run_mycelium(home.path(), &["echo", "hello"]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("plugin-error"),
        "stderr should include plugin stderr: {stderr}"
    );
    assert!(
        stderr.contains("Plugin exited with non-zero status"),
        "stderr should include the plugin failure path: {stderr}"
    );
    assert!(
        !stderr.contains("PLUGIN OUTPUT"),
        "discarded plugin stdout must not replace the original output: {stderr}"
    );
}
