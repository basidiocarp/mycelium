# Codex Runtime Interception Gap

## Summary

Mycelium discovered that Codex does not currently support transparent runtime interception of shell commands. This document explains what was investigated, why each approach is blocked, what Mycelium already provides, and what Codex would need to add to enable this workflow.

## Investigation Scope

This analysis examined three potential runtime interception mechanisms in Codex:

1. **Shell environment override** (`SHELL` or `CODEX_SHELL` variable)
2. **Pre-execution hook** (`pre_exec` callback in config)
3. **Custom tool registration** (exec_filter or tool wrapper registration)

## Findings

### Shell Environment Override - NOT SUPPORTED

Codex does not read or respect `SHELL`, `CODEX_SHELL`, or similar environment variables to override the shell used for command execution. The Codex configuration schema (`~/.codex/config.toml`) supports:

- Model selection (`model`, `model_reasoning_effort`)
- Sandbox and approval modes
- MCP server registration
- Profiles and multi-agent setup
- Feature flags and UI settings

But not shell selection or override.

### Pre-Execution Hook - NOT SUPPORTED

Codex configuration does not provide a `pre_exec`, `pre_hook`, or similar callback mechanism. Commands are executed directly via the native shell without observable interception points.

The documented execution model for external tools is one-way: `codex exec` invokes the tool explicitly with a prompt, but there's no callback from Codex back to the shell.

### Custom Tool Registration - NOT SUPPORTED

Codex does not expose an `exec_filter` or mechanism to register a custom command wrapper that intercepts execution. MCP servers can provide new tools, but they cannot wrap the native shell's `exec_command` or similar function calls.

## What Mycelium Already Provides

Mycelium offers **retrospective command discovery** for Codex sessions:

- `mycelium discover` scans `~/.codex/sessions/` JSONL files
- Extracts all executed commands and their output
- Categorizes which commands could benefit from Mycelium compression
- Reports missed token savings by command family
- Works for both Claude Code and Codex CLI

**Precondition**: Codex must have run at least one session before `mycelium discover` returns results. The `~/.codex/sessions/` directory is created by Codex on first use; if it does not exist, `mycelium discover` returns an empty result set rather than an error.

This provides visibility into Codex usage patterns without requiring runtime interception.

## What Codex Would Need to Add

To enable transparent runtime interception, Codex would need one of these:

### Option A: Shell Override Environment Variable

```bash
export MYCELIUM_SHELL=true
export CODEX_SHELL_WRAPPER=/usr/local/bin/mycelium
codex exec <prompt>
```

The Codex execution engine would check for these variables and use the specified wrapper instead of the default shell.

**Impact**: Low; non-breaking change to Codex's shell initialization.

### Option B: Pre-Execution Hook in config.toml

```toml
[hooks.pre_exec]
command = "/usr/local/bin/mycelium"
pass_through = true
```

Before executing any shell command, Codex would invoke the hook with the full command as input and expect either:
- The hook's stdout (filtered output), or
- An exit code indicating "pass through unchanged"

**Impact**: Moderate; requires hook lifecycle management in the Codex execution engine.

### Option C: MCP Tool Wrapper Registration

Register Mycelium as an MCP server that provides a `filter_and_exec` tool. This is the highest-friction option because MCP tools are invoked by the **model** (via tool-call JSON), not by the Codex host process itself. Runtime shell interception requires the host to redirect execution; an MCP tool can only be called when the model explicitly requests it.

This would work only if Codex's host process were extended to route its native shell execution through an MCP tool call — which is architecturally equivalent to Option A or B but with MCP as the transport layer.

```toml
[mcp_servers.mycelium]
command = "mycelium"
args = ["mcp-serve"]
```

**Impact**: Very high; requires Codex to change its execution model and the model to explicitly call the wrapper tool on every shell invocation.

## Recommendation

**For Codex maintainers**: Option A (shell override environment variable) is the simplest and least invasive. It respects Codex's execution model while enabling third-party tools like Mycelium to participate transparently.

**For Mycelium users**: Until Codex adds one of these mechanisms, use `mycelium discover` to identify high-value opportunities for Mycelium adoption. The retrospective analysis is accurate and requires no runtime changes.

## See Also

- `mycelium discover` — analyze Codex sessions for missed savings
- `~/.codex/config.toml` — Codex configuration reference
- `mycelium/src/discover/provider/codex.rs` — implementation of session discovery
