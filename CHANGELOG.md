# Changelog

All notable changes to Mycelium are documented in this file.

## [Unreleased]

### Changed

- **Changelog format**: Release headings and entry structure now follow the
  shared ecosystem changelog template.

## [0.8.10] - 2026-04-08

### Changed

- **Foundation alignment**: README, architecture notes, plugin docs, and
  command guidance now describe Mycelium's internal module boundaries and
  sibling-tool responsibilities more explicitly.
- **Boundary verification**: Added foundation alignment coverage for dispatch,
  tracking, and integration-layer structure.

### Fixed

- **Plugin fallback safety**: fallback commands keep raw command execution
  single-shot and preserve stderr and exit status on plugin failure.
- **Tracing depth**: onboarding, Rhizome, plugin, and tracking paths now carry
  the shared workflow and subprocess context deeper into the touched runtime
  boundaries.

## [0.8.9] - 2026-04-08

### Fixed

- **Safe plugin fallback semantics**: plugin-backed fallback commands now run
  the underlying command once, replay captured stdout/stderr on plugin failure,
  and no longer risk masking real failures or executing side-effecting commands
  twice.
- **Broader shared tracing coverage**: Rhizome calls, onboarding MCP flows,
  fallback dispatch, and tracking writes now enter shared workflow, tool, or
  subprocess spans instead of stopping at startup and Hyphae chunking.
- **Better subprocess diagnostics**: onboarding and plugin/runtime subprocesses
  preserve child stderr at useful failure points instead of silently dropping it.
- **Plugin docs now match the implementation**: the plugin contract and command
  reference now describe the real ownership checks, shipped-template behavior,
  and raw replay semantics.

## [0.8.8] - 2026-04-08

### Changed

- **Shared logging rollout**: Mycelium now initializes logging through Spore's
  app-aware `MYCELIUM_LOG` path instead of relying on generic runtime setup.
- **Workflow tracing**: Startup, Hyphae subprocess launches, and Hyphae-backed
  tool calls now emit shared tracing spans with workspace-aware context for
  faster failure localization.

### Fixed

- **Operator guidance**: Docs now distinguish debug logging on stderr from
  Mycelium's normal command output, audit messages, and passthrough stderr.

## [0.8.7] - 2026-04-01

### Added

- **Filter quality tracking**: `FilterQuality` now flows through the
  route-or-filter pipeline so Mycelium can report honest filter quality instead
  of assuming full success.
- **Degraded validation rule**: Added explicit degraded-quality validation for
  the new quality model.

### Changed

- **GitHub CLI quality reporting**: `gh` handlers now carry `FilterResult`
  quality data and route output through validation and Hyphae chunking more
  consistently.

## [0.8.3] - 2026-04-01

### Fixed

- **Diagnostic passthrough routing**: Diagnostic shell commands now stay on the
  raw invoke passthrough path instead of producing misleading savings estimates.
- **Small-output passthrough**: Outputs of five lines or fewer now stay on the
  passthrough path even when their byte size is large.

## [0.8.2] - 2026-04-01

### Added

- **Safe find rewrites**: Safe `find` commands can now rewrite to `fd`.

## [0.8.1] - 2026-04-01

### Fixed

- **`ls` metadata passthrough**: Diagnostic `ls` metadata flags now stay on the
  passthrough path.

## [0.8.0] - 2026-03-31

### Changed

- **Token-optimizer boundary**: Mycelium now stays focused on token
  optimization, analytics, and Hyphae interaction while Stipe owns shared
  ecosystem setup and host mutation.
- **Shared MCP transport**: Hyphae communication now uses Spore's MCP client
  and the published ecosystem error envelope.
- **Versioned integration boundaries**: Hyphae command-output writes and
  Cap-facing gain and history reads now use explicit schema-versioned contracts
  with identity-v1 fields.

### Fixed

- **Private coupling removal**: Cap no longer depends on Mycelium's private
  SQLite or config layouts for analytics and history.
- **Runtime session propagation**: Hyphae-bound command-output storage now
  forwards runtime session ids so retrieval can join correctly with the active
  session timeline.

## [0.7.4] - 2026-03-28

### Changed

- **Claude host capability model**: `mycelium init`, `mycelium doctor`, and
  related setup paths now use an explicit supported-versus-unsupported Claude
  capability boundary.
- **Cleaner unsupported-host fallback**: Non-Unix environments now report the
  Claude Code Bash hook adapter as unsupported while treating
  `mycelium init -g --claude-md` as the supported global docs-only fallback.
- **More accurate setup guidance**: CLI help, ecosystem docs, and update docs
  now distinguish hook-adapter setup from docs-only `CLAUDE.md` setup.

### Fixed

- **Docs-only uninstall gap**: `mycelium init -g --uninstall` now removes
  legacy global `CLAUDE.md` instructions in addition to hook artifacts.
- **Doctor fallback visibility**: `mycelium doctor` now has a healthy path for
  the supported global docs-only Claude fallback.

## [0.7.3] - 2026-03-28

### Changed

- **Shell parser backend**: Rewrite safety validation now uses
  `tree-sitter-bash` instead of the deprecated vendored `conch-parser`.
- **Rewrite docs alignment**: Architecture, analytics, and feature docs now
  describe parser-backed shell safety instead of the old regex-only rewrite
  model.

## [0.7.2] - 2026-03-26

### Fixed

- **Platform-aware hook audit paths**: `mycelium hook-audit` now uses the
  shared platform data directory instead of Unix-shaped fallback paths.
- **Unified shell resolution**: Runtime shell execution now goes through one
  shared platform-aware resolver instead of assuming direct `sh` execution.

## [0.7.1] - 2026-03-26

### Changed

- **Cross-platform runtime plumbing**: Added a shared platform layer for
  config and data paths, shell dispatch, PATH parsing, and command lookup.
- **Shared editor registration**: `mycelium init` now uses Spore's shared
  editor and config model for Codex CLI, Cursor, Windsurf, and Claude Desktop.
- **Host-neutral setup guidance**: Onboarding, ecosystem setup, and doctor
  output now present Claude Code and Codex CLI as peer host adapters.

## [0.7.0] - 2026-03-23

### Added

- **Codex-aware ecosystem setup**: `mycelium init --ecosystem` now recognizes
  Codex CLI as a first-class host client and can register Hyphae and Rhizome
  MCP servers into `~/.codex/config.toml`.

### Changed

- **Host-aware onboarding**: `mycelium init --onboard` now guides users through
  configuring the detected host client instead of assuming Claude Code is
  mandatory.
- **Host-specific next steps**: Onboarding summaries and help text now show
  Claude and Codex follow-up steps based on the clients detected on the
  machine.

## [0.6.0] - 2026-03-23

### Added

- **Task-runner-aware rewrite support**: Explicit wrapper forms such as
  `mise exec -- <command>`, `just -- <command>`, and `task -- <command>` now
  unwrap to the underlying command for rewrite and classification.
- **Tracking DB status view**: `mycelium gain --status` now reports the active
  tracking database path, its source, and basic health details.
- **Curated library API**: `src/lib.rs` now exposes rewrite, filter, compaction,
  and tracking helpers for downstream Basidiocarp tools.

### Changed

- **Broader `gh` edge-mode passthrough**: Issue, PR, run, and repo views now
  pass through more output-shaping modes such as `--json`, `--jq`, `--web`, and
  `--comments` instead of trying to filter the wrong output shape.
- **Named compaction profiles**: Added `debug`, `balanced`, and `aggressive`
  compaction profiles and wired them into adaptive classification and git
  compression budgets.
- **Hook install diagnostics**: Installed rewrite hooks now carry a stamped
  Mycelium version, and `mycelium init --show-config` can distinguish current
  hooks from stale or unknown ones.

### Fixed

- **Hook repair visibility**: Rewrite hooks now explain skipped rewrites more
  clearly when `mycelium`, `jq`, or embedded paths are stale or missing.
- **Tracking path drift diagnosis**: Config, doctor output, and tracking
  utilities now surface whether the active DB path came from an override, env
  var, config file, or platform default.

## [0.5.1] - 2026-03-23

### Changed

- **`gh` passthrough edge modes**: `mycelium gh issue view` and related GH
  commands now defer to the real GitHub CLI for browser, template, JSON, and
  comment modes.
- **Larger Git compaction budgets**: Retained diff hunk and status-file budgets
  were increased so routine repository state is less likely to be truncated.
- **Rewrite hook installation**: Installed hooks now embed resolved `mycelium`
  and `jq` paths while still falling back to `PATH` when needed.

### Fixed

- **Signed commit coverage**: Added regression coverage around `git commit -S`
  flows so signed commit and signed amend behavior stays intact.
- **Hook PATH fragility**: Rewrite hooks now emit explicit diagnostics when
  runtime dependencies are unavailable.

## [0.5.0] - 2026-03-22

### Added

- **`mycelium rewrite --explain`**: Added a surface that shows whether a
  command rewrites through the built-in registry, a learned correction, or not
  at all, with the reason for that decision.

### Changed

- **Onboarding handoff to Stipe**: Setup and update docs now point at
  `stipe init` as the primary onboarding and repair path, while
  `mycelium init --ecosystem` remains the lower-level integration path.

## [0.4.5] - 2026-03-21

### Changed

- **Dispatch refactor**: `dispatch()` was decomposed into focused per-family
  helpers such as git, gh, cargo, and docker.
- **Tracker reuse**: `record_parse_failure_silent` now accepts an optional
  tracker reference to avoid double SQLite opens on fallback paths.
- **Hook replacement**: Deprecated JS and shell capture hooks were removed in
  favor of Cortina.
- **Shared Spore runtime**: Self-update and token estimation now use shared
  Spore modules.

### Fixed

- **Plugin PID race condition**: Timeout handling now checks cancellation before
  sending signals, preventing stray signals to recycled PIDs.
- **Plugin ownership check**: Production ownership checks now use a real UID
  path instead of shell-specific environment variables.
- **Omission marker counts**: `smart_truncate` now reports the actual section
  size.

## [0.3.2] - 2026-03-18

### Added

- **Interactive onboarding**: Added `mycelium init --onboard` to guide new
  users through ecosystem setup, tool detection, and configuration.
- **Multi-client ecosystem init**: Added `mycelium init --ecosystem --client
  <name>` for separate MCP config paths per client.
- **Context command**: Added `mycelium context <task>` to gather relevant
  context from Hyphae, Rhizome, and local project state.
- **Session-summary stop hook**: Added automatic session summaries when Claude
  Code exits.

### Changed

- **Shared tool discovery**: Remaining manual binary detection moved to Spore's
  shared `discover()` API.

### Fixed

- **Config parsing coverage**: Added config-deserialization coverage for edge
  cases and cleaned pedantic clippy warnings across the codebase.

## [0.2.2] - 2026-03-16

### Added

- **Ecosystem init**: `mycelium init --ecosystem` can now detect sibling tools
  such as Hyphae, Rhizome, and Cap and register their MCP servers with Claude
  Code in one command.

### Changed

- **Shared discovery**: Hyphae and Rhizome module detection now uses Spore's
  shared tool-discovery API instead of manual `which` and `where` probing.
- **CI workflow refresh**: CI configuration was updated for the new ecosystem
  setup flow.

### Fixed

- **Hyphae-aware tests**: Tests now adapt when Hyphae is already installed on
  the machine instead of assuming it is absent.

## [0.2.1] - 2026-03-16

### Added

- **Hyphae integration**: Large command outputs can now be chunked into Hyphae
  instead of being destructively filtered, with retrieval keys returned to the
  agent.
- **Rhizome integration**: `mycelium read` can now delegate code files to
  Rhizome for structural summaries instead of applying local filters.

### Changed

- **CI streamlining**: Concurrency groups, prebuilt tool installs, merged
  performance jobs, and coverage cleanup landed with the integration release.

## [0.2.0] - 2026-03-15

### Added

- **Adaptive filtering**: Small outputs pass through unfiltered, medium outputs
  get light filtering, and large outputs get full structured compression.
- **Comment classification**: MinimalFilter now distinguishes actionable
  comments from noise.
- **License header detection**: Large comment-only preambles are now detected
  and stripped.
- **Function body folding**: AggressiveFilter now folds large function bodies
  instead of dropping them outright.

### Changed

- **Tool-specific tuning**: Curl, docker logs, git diff, git log, git status,
  and test formatters all received less destructive defaults and fuller error
  output.
- **Docs refresh**: Architecture, features, commands, README, and planning docs
  were updated to explain adaptive filtering and the optional Hyphae and
  Rhizome integrations.
- **CI refresh**: Concurrency groups, prebuilt tool installs, merged
  performance jobs, and upload-artifact updates landed with the release.

## [0.1.6] - 2026-03-11

### Security

- **Plugin PID reuse race**: Plugin timeout handling now kills the actual child
  process instead of relying on raw PID signaling.
- **UID ownership checks**: Plugin security checks now use portable UID
  detection and fail closed on error.

### Changed

- **Cleanup scheduling**: Tracking cleanup now runs at most once per day
  instead of after every command.
- **Schema init caching**: Migrations now skip when the schema version is
  current.

### Fixed

- **Summary operator precedence**: Test-result detection no longer
  misclassifies commands because of `||` and `&&` precedence.

## [0.1.5] - 2026-03-11

### Added

- **Benchmark command**: Added `mycelium benchmark`, including a CI mode that
  can fail when too few tests show savings.
- **Plugin management**: Added `mycelium plugin list` and
  `mycelium plugin install <name>`.
- **Per-project analytics**: Added `mycelium gain --project` and
  `mycelium gain --projects`.
- **Enhanced doctor checks**: Doctor output now validates hook registration,
  plugin-directory state, and PATH configuration.

### Changed

- **Safer error handling**: Production `.unwrap()` calls were replaced with
  safer propagation or defaults.
- **Git stash filtering**: `git stash list` now strips verbose branch and date
  prefixes while keeping the useful stash index and message.
- **Hook template v3**: The rewrite hook template gained better version guards,
  jq error handling, heredoc safety, and opt-in audit logging.
- **Coverage guard**: CI now enforces a minimum code-coverage target.

### Fixed

- **Hook jq handling**: Malformed jq handling that caused silent hook failures
  was fixed.
- **Source formatting**: Formatting inconsistencies across multiple source files
  were corrected.

## [0.1.4] - 2026-03-11

### Changed

- **Self-update command**: `mycelium self-update` was overhauled with improved
  error handling and release detection.

### Fixed

- **Latest-release detection**: The updater now detects the newest GitHub
  release correctly.

## [0.1.3] - 2026-03-11

### Added

- **Release script**: Added `scripts/release.sh` for version bumps, tagging,
  and GitHub release creation.
- **CLI output cleanup**: Help text and command display formatting were
  improved.

### Fixed

- **Release script version handling**: The release script now handles version
  resolution correctly.

## [0.1.2] - 2026-03-10

### Changed

- **CLI cleanup**: Subcommands and help text were reorganized and standardized.
- **Cross-platform fixes**: Windows build failures and ETXTBSY-related Linux CI
  issues were addressed.
- **Installation improvements**: The install script and verification checks were
  updated.

### Fixed

- **Windows build errors**: Windows-specific build failures were corrected.
- **Install path detection**: Installation path detection now resolves the
  intended location more reliably.

## [0.1.1] - 2026-03-10

### Changed

- **Learning system fixes**: `mycelium learn` now detects error corrections
  more reliably.
- **CI hardening**: GitHub Actions dependencies were updated, clippy warnings
  were cleaned up, and shell formatting was standardized.

### Fixed

- **Cross-platform CI failures**: Windows dead-code warnings and Linux ETXTBSY
  CI failures were corrected.

## [0.1.0] - 2026-03-10

### Added

- **CLI proxy foundation**: Mycelium shipped as a token-optimized CLI proxy for
  Claude Code and related development workflows.
- **Wide filter catalog**: The initial release included 45 or more command
  filters across Git, GitHub, Cargo, Docker, AWS, Terraform, and other common
  categories.
- **Hook-based rewriting**: Claude Code command rewriting shipped in the first
  release.
- **Savings analytics**: Added `mycelium gain` for token-savings reporting.
- **Discovery surface**: Added `mycelium discover` for opportunity discovery.
- **Self-update support**: The initial release shipped with self-update
  capabilities.
- **Cross-platform support**: macOS, Linux, and Windows support were part of
  the first release.
- **CI baseline**: Formatting, linting, testing, performance guards, and
  security checks shipped with the initial release.
