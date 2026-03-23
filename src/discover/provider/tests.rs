use super::*;
use std::fs;
use std::io::Write;

fn make_jsonl(lines: &[&str]) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    for line in lines {
        writeln!(f, "{}", line).unwrap();
    }
    f.flush().unwrap();
    f
}

#[test]
fn test_extract_assistant_bash() {
    let jsonl = make_jsonl(&[
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_abc","name":"Bash","input":{"command":"git status"}}]}}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_abc","content":"On branch master\nnothing to commit"}]}}"#,
    ]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].command, "git status");
    assert!(cmds[0].output_len.is_some());
    assert_eq!(
        cmds[0].output_len.unwrap(),
        "On branch master\nnothing to commit".len()
    );
}

#[test]
fn test_extract_non_bash_ignored() {
    let jsonl = make_jsonl(&[
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_abc","name":"Read","input":{"file_path":"/tmp/foo"}}]}}"#,
    ]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 0);
}

#[test]
fn test_extract_non_message_ignored() {
    let jsonl =
        make_jsonl(&[r#"{"type":"file-history-snapshot","messageId":"abc","snapshot":{}}"#]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 0);
}

#[test]
fn test_extract_multiple_tools() {
    let jsonl = make_jsonl(&[
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"git status"}},{"type":"tool_use","id":"toolu_2","name":"Bash","input":{"command":"git diff"}}]}}"#,
    ]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0].command, "git status");
    assert_eq!(cmds[1].command, "git diff");
}

#[test]
fn test_extract_malformed_line() {
    let jsonl = make_jsonl(&[
        "this is not json at all",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_ok","name":"Bash","input":{"command":"ls"}}]}}"#,
    ]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].command, "ls");
}

#[test]
fn test_encode_project_path() {
    assert_eq!(
        ClaudeProvider::encode_project_path("/Users/foo/bar"),
        "-Users-foo-bar"
    );
}

#[test]
fn test_encode_project_path_trailing_slash() {
    assert_eq!(
        ClaudeProvider::encode_project_path("/Users/foo/bar/"),
        "-Users-foo-bar-"
    );
}

#[test]
fn test_match_project_filter() {
    let encoded = ClaudeProvider::encode_project_path("/Users/foo/Sites/mycelium");
    assert!(encoded.contains("mycelium"));
    assert!(encoded.contains("Sites"));
}

#[test]
fn test_extract_output_content() {
    let jsonl = make_jsonl(&[
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_abc","name":"Bash","input":{"command":"git commit --ammend"}}]}}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_abc","content":"error: unexpected argument '--ammend'","is_error":true}]}}"#,
    ]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].command, "git commit --ammend");
    assert!(cmds[0].is_error);
    assert!(cmds[0].output_content.is_some());
    assert_eq!(
        cmds[0].output_content.as_ref().unwrap(),
        "error: unexpected argument '--ammend'"
    );
}

#[test]
fn test_extract_is_error_flag() {
    let jsonl = make_jsonl(&[
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls"}},{"type":"tool_use","id":"toolu_2","name":"Bash","input":{"command":"invalid_cmd"}}]}}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"file1.txt","is_error":false},{"type":"tool_result","tool_use_id":"toolu_2","content":"command not found","is_error":true}]}}"#,
    ]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 2);
    assert!(!cmds[0].is_error);
    assert!(cmds[1].is_error);
}

#[test]
fn test_extract_sequence_ordering() {
    let jsonl = make_jsonl(&[
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"first"}},{"type":"tool_use","id":"toolu_2","name":"Bash","input":{"command":"second"}},{"type":"tool_use","id":"toolu_3","name":"Bash","input":{"command":"third"}}]}}"#,
    ]);

    let provider = ClaudeProvider;
    let cmds = provider.extract_commands(jsonl.path()).unwrap();
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[0].sequence_index, 0);
    assert_eq!(cmds[1].sequence_index, 1);
    assert_eq!(cmds[2].sequence_index, 2);
    assert_eq!(cmds[0].command, "first");
    assert_eq!(cmds[1].command, "second");
    assert_eq!(cmds[2].command, "third");
}

#[test]
fn test_codex_extract_exec_command() {
    let jsonl = make_jsonl(&[
        r#"{"timestamp":"2026-03-23T22:32:35.887Z","type":"session_meta","payload":{"id":"019d1cd4-1b21-7501-9b26-8ceeab64b535","cwd":"/Users/foo/mycelium"}}"#,
        r#"{"timestamp":"2026-03-23T22:32:35.888Z","type":"response_item","payload":{"type":"function_call","call_id":"call_abc","name":"exec_command","arguments":"{\"cmd\":\"git status\",\"workdir\":\"/Users/foo/mycelium\",\"max_output_tokens\":200}"}}"#,
        r#"{"timestamp":"2026-03-23T22:32:35.889Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_abc","output":"Command: /bin/zsh -lc git status\nChunk ID: abc123\nWall time: 0.0000 seconds\nProcess exited with code 1\nOutput:\nfatal: not a git repository\n","error":null,"status":null}}"#,
    ]);

    let cmds = CodexProvider
        .extract_commands(jsonl.path())
        .expect("codex commands");

    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].command, "git status");
    assert!(cmds[0].is_error);
    assert_eq!(
        cmds[0].output_len,
        Some("fatal: not a git repository\n".len())
    );
    assert_eq!(
        cmds[0].output_content.as_deref(),
        Some("fatal: not a git repository\n")
    );
}

#[test]
fn test_codex_source_label() {
    assert_eq!(SessionSource::ClaudeCode.label(), "Claude Code");
    assert_eq!(SessionSource::CodexCli.label(), "Codex CLI");
}

#[test]
fn test_project_filter_for_source_defaults() {
    let cwd = "/Users/foo/mycelium";
    assert_eq!(
        project_filter_for_source(SessionSource::ClaudeCode, None, false, cwd).as_deref(),
        Some("-Users-foo-mycelium")
    );
    assert_eq!(
        project_filter_for_source(SessionSource::CodexCli, None, false, cwd).as_deref(),
        Some("/Users/foo/mycelium")
    );
    assert_eq!(
        project_filter_for_source(SessionSource::ClaudeCode, Some("foo"), false, cwd).as_deref(),
        Some("foo")
    );
    assert_eq!(
        project_filter_for_source(
            SessionSource::ClaudeCode,
            Some("/Users/foo/mycelium"),
            false,
            cwd
        )
        .as_deref(),
        Some("-Users-foo-mycelium")
    );
    assert_eq!(
        project_filter_for_source(SessionSource::CodexCli, None, true, cwd),
        None
    );
}

#[test]
fn test_codex_project_filter_matches_path_boundaries() {
    assert!(shared::codex_project_filter_matches(
        "/Users/foo/mycelium",
        "/Users/foo/mycelium"
    ));
    assert!(shared::codex_project_filter_matches(
        "/Users/foo/mycelium/subdir",
        "/Users/foo/mycelium"
    ));
    assert!(!shared::codex_project_filter_matches(
        "/Users/foo/mycelium-old",
        "/Users/foo/mycelium"
    ));
    assert!(shared::codex_project_filter_matches(
        "/Users/foo/work/mycelium",
        "mycelium"
    ));
    assert!(!shared::codex_project_filter_matches(
        "/Users/foo/work/mycelium-old",
        "mycelium"
    ));
}

#[cfg(unix)]
#[test]
fn test_codex_discover_skips_unreadable_session_meta_and_keeps_valid_match() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempfile::tempdir().unwrap();
    let valid_path = temp_dir.path().join("valid.jsonl");
    let unreadable_path = temp_dir.path().join("unreadable.jsonl");

    fs::write(
        &valid_path,
        concat!(
            r#"{"timestamp":"2026-03-23T22:32:35.887Z","type":"session_meta","payload":{"id":"session-1","cwd":"/Users/foo/mycelium"}}"#,
            "\n",
            r#"{"timestamp":"2026-03-23T22:32:35.888Z","type":"response_item","payload":{"type":"function_call","call_id":"call_abc","name":"exec_command","arguments":"{\"cmd\":\"git status\"}"}}"#,
            "\n"
        ),
    )
    .unwrap();
    symlink(temp_dir.path().join("missing.jsonl"), &unreadable_path).unwrap();

    let sessions =
        CodexProvider::discover_sessions_in(temp_dir.path(), Some("/Users/foo/mycelium"), None)
            .unwrap();

    assert_eq!(sessions, vec![valid_path]);
}
