# FlowForge Setup Guide

## Quick Start

```bash
# From the flowforge repo root:
./setup.sh
```

This builds, installs, and initializes FlowForge for the current project.

## What `setup.sh` Does

1. **Builds** FlowForge in release mode (`cargo build --release`)
2. **Installs** the `flowforge` binary to `~/.cargo/bin/`
3. **Initializes** the current project with `flowforge init --project`, which creates:

| File | Purpose | Git tracked? |
|------|---------|-------------|
| `.flowforge/config.toml` | Project config (routing weights, pattern settings, etc.) | No (`.gitignore`) |
| `.flowforge/flowforge.db` | SQLite database (sessions, patterns, work items) | No (`.gitignore`) |
| `.flowforge/agents/` | Directory for project-specific custom agents | No (`.gitignore`) |
| `.claude/settings.json` | Claude Code hooks (13 hook events wired to FlowForge) | **Yes** |
| `.mcp.json` | MCP server auto-registration | **Yes** |
| `CLAUDE.md` | Agent orchestration instructions for Claude | **Yes** |

## Prerequisites

- Rust toolchain (`rustup` + `cargo`)
- `~/.cargo/bin` on your `PATH`
- SQLite3 (comes with macOS, install via package manager on Linux)

## Manual Setup

If you prefer not to use the script:

```bash
# Build
cargo build --release

# Install
cargo install --path crates/flowforge-cli

# Initialize for your project
cd /path/to/your/project
flowforge init --project

# Optional: global config
flowforge init --global
```

## Setting Up a Different Project

FlowForge can be initialized in any project directory:

```bash
cd /path/to/other/project
flowforge init --project
```

This is safe to run multiple times — it merges into existing `.claude/settings.json` rather than overwriting.

## Verifying the Setup

```bash
# Check hooks work
echo '{"prompt":"test"}' | flowforge hook user-prompt-submit

# Check agents are loaded (should show 60)
flowforge agent list | tail -1

# Check session tracking
flowforge session current

# Check work tracking
flowforge work status

# Check routing
flowforge route "fix a bug"
```

## Architecture

```
flowforge (CLI binary, ~7.5MB)
├── flowforge-cli      # CLI commands + 13 hook handlers
├── flowforge-core     # Config, types, hook I/O, work tracking
├── flowforge-memory   # SQLite DB, HNSW vectors, pattern learning
├── flowforge-agents   # 60 built-in agents, registry, router
├── flowforge-mcp      # MCP server (22 tools over JSON-RPC 2.0)
└── flowforge-tmux     # tmux team monitor
```

## Hook Events

All 13 Claude Code hook events are wired:

| Event | What FlowForge Does |
|-------|-------------------|
| `SessionStart` | Creates session record, syncs work items from external backends |
| `SessionEnd` | Ends session, pushes work items to backends, runs pattern consolidation |
| `UserPromptSubmit` | Routes to best agent, injects context about active work items |
| `PreToolUse` | Blocks dangerous bash commands (rm -rf /, fork bombs, etc.) |
| `PostToolUse` | Tracks file edits (Write/Edit/MultiEdit operations) |
| `PostToolUseFailure` | Records error patterns for learning |
| `PreCompact` | Injects guidance before context compaction |
| `SubagentStart` | Updates tmux monitor, assigns work item to agent |
| `SubagentStop` | Updates tmux monitor, extracts patterns from agent output |
| `TeammateIdle` | Updates tmux monitor status |
| `TaskCompleted` | Updates routing weights (learning), logs work events |
| `Stop` | Ends active session |
| `Notification` | Logs notifications to audit trail |

## MCP Tools (22)

Available when Claude connects to the FlowForge MCP server:

**Memory:** `memory_get`, `memory_set`, `memory_delete`, `memory_list`, `memory_search`, `memory_import`
**Learning:** `learning_store`, `learning_search`, `learning_feedback`, `learning_stats`
**Agents:** `agents_list`, `agents_info`, `agents_route`
**Sessions:** `session_status`, `session_history`, `session_metrics`
**Team:** `team_status`, `team_log`
**Work:** `work_create`, `work_list`, `work_update`, `work_log`

## Troubleshooting

### Hooks not firing
- Check `which flowforge` returns a path
- Verify `.claude/settings.json` has the hook entries
- Check `.flowforge/hook-errors.log` for errors

### "FlowForge not initialized" errors
- Run `flowforge init --project` in your project root

### MCP server not connecting
- Check `.mcp.json` exists with the flowforge entry
- Restart Claude Code session after adding `.mcp.json`

### Work tracking backend
- FlowForge auto-detects: `.kanbus.yml` → Kanbus, `.beads/` → Beads, else → Claude Tasks
- Override in `.flowforge/config.toml`: `[work_tracking] backend = "kanbus"`
