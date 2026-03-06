# Contributing to FlowForge

This document describes what to update when making changes to FlowForge. It serves as a checklist so nothing gets missed.

## Build & Verify

After any change, always run:

```bash
cargo build --workspace              # Must compile with 0 warnings
cargo test --workspace               # All 408+ tests must pass
cargo clippy --workspace -- -D warnings  # Must be clean
cargo fmt --all --check              # Must be formatted
```

Install locally to test hooks and CLI:

```bash
cargo install --path crates/flowforge-cli --force
```

## Adding a New MCP Tool

MCP tools are how Claude interacts with FlowForge during a session.

1. **Register the tool** in the appropriate `crates/flowforge-mcp/src/tools/<category>.rs` file inside `register()`:
   - Add a `self.register(ToolDef { name, description, input_schema })` call
   - Keep tools grouped by category; create a new file for new categories

2. **Handle the tool call** in the same file's `call()` method:
   - Add a match arm for the tool name
   - Parse parameters using `ParamExt` trait methods (use `require_str()` for required params)
   - Return JSON result

3. **Update the tool count test** in `crates/flowforge-mcp/src/server.rs`:
   - Find `assert_eq!(tools.len(), 53)` and increment the count

4. **Update documentation**:
   - `README.md` — MCP Tools table, feature count in intro
   - `SETUP.md` — MCP Tools section
   - `CLAUDE.md` — if it adds a new user-facing capability

## Adding a New CLI Command

1. **Add the subcommand enum variant** in `crates/flowforge-cli/src/main.rs`:
   - For a new top-level command: add to `Commands` enum
   - For a subcommand: add to the relevant `*Action` enum (e.g., `WorkAction`, `LearnAction`)

2. **Add the match arm** in `main()` to dispatch to the handler function

3. **Implement the handler** in `crates/flowforge-cli/src/commands/`:
   - For new command groups: create a new file and add `pub mod <name>;` to `commands/mod.rs`
   - For existing groups: add a function to the existing file

4. **Update documentation**:
   - `README.md` — CLI Reference table, Usage examples
   - `SETUP.md` — relevant section

## Adding a New Hook Behavior

Hooks are the main integration point between Claude Code and FlowForge. They run on every tool use, session start/end, etc.

1. **Modify the hook handler** in `crates/flowforge-cli/src/hooks/<event>.rs`

2. **Check hook wiring** in `.claude/settings.json`:
   - PreToolUse and PostToolUse must have **no** `"matcher"` field — they fire for ALL tools
   - If adding a new hook event, add it to `flowforge_hooks()` in `crates/flowforge-cli/src/commands/init.rs`

3. **Keep hooks fast**: hooks block Claude Code. Target <50ms for PreToolUse, <200ms for others. Use `run_safe()` wrapper so failures don't break the session.

4. **Run `flowforge test-hooks`** to verify all 13 hooks pass with realistic Claude Code payloads (includes extra fields like `hook_event_name`, `permission_mode`, `tool_use_id`).

5. **Update documentation**:
   - `README.md` — Hooks table
   - `SETUP.md` — Hook Events table

## Adding a New DB Table or Column

1. **Add the table** to `init_schema()` in `crates/flowforge-memory/src/db/schema.rs`:
   - New tables: add `CREATE TABLE IF NOT EXISTS` statement
   - New columns on existing tables: use `migrate_add_column()` pattern (ALTER TABLE with IF NOT EXISTS check)
   - Add indexes for frequently-queried columns
   - Bump `SCHEMA_VERSION` when adding indexes or migrations

2. **Add DB methods** in the appropriate `crates/flowforge-memory/src/db/<module>.rs` file:
   - `sessions.rs` for session operations
   - `work_items.rs` for work item CRUD and work-stealing
   - `agent_sessions.rs` for agent session management
   - Follow the existing pattern: `pub fn method_name(&self, ...) -> Result<T>`
   - Use `self.conn.execute()` for writes, `self.conn.query_row()` / `self.conn.prepare()` for reads
   - For multi-step mutations, use `self.with_transaction(|| { ... })` for atomicity

3. **If the table is accessed by both CLI and MCP**, add the method to the `WorkDb` trait in `crates/flowforge-core/src/work_tracking/mod.rs` and implement it for `MemoryDb`

## Adding a New Type

1. **Add the type** to the appropriate `crates/flowforge-core/src/types/<module>.rs` file:
   - `work.rs` for work items, events, filters
   - `sessions.rs` for session types
   - `agents.rs` for agent definitions
   - `guidance.rs` for guidance/gate types
   - `patterns.rs` for pattern learning types
   - `collaboration.rs` for mailbox, team, checkpoint types
   - Use `#[derive(Debug, Clone, Serialize, Deserialize)]` for data types
   - Use `#[derive(Debug, Clone, Copy, PartialEq)]` for enums

2. **Re-export** from `crates/flowforge-core/src/types/mod.rs` — types are automatically re-exported from the crate root

## Adding a New Config Section

1. **Add the config struct** to `crates/flowforge-core/src/config.rs`:
   - Add a new struct with `#[derive(Debug, Clone, Serialize, Deserialize)]`
   - Add a field to `FlowForgeConfig` with `#[serde(default)]`
   - Implement `Default` with sensible defaults

2. **Update documentation**:
   - `SETUP.md` — Configuration section with example TOML

## Adding a New Core Module

1. **Create the file** in `crates/flowforge-core/src/<module>.rs`
2. **Add to lib.rs**: `pub mod <module>;` in `crates/flowforge-core/src/lib.rs`
3. **If it has a memory component**, also create `crates/flowforge-memory/src/<module>.rs` and add to that crate's `lib.rs`

## Adding a New Agent

1. **Create the markdown file** in `agents/<category>/<name>.md`
2. The agent registry auto-discovers agents from the `agents/` directory
3. If adding a new category directory, no code changes needed — the registry walks subdirectories

## Adding a New Plugin

Plugins don't require code changes. Create the plugin directory:

```
.flowforge/plugins/<name>/
├── plugin.toml
├── scripts/
│   └── tool.py
└── agents/
    └── agent.md
```

See `README.md` Plugin SDK section for the `plugin.toml` format.

## Adding a New Error Variant

1. **Add the variant** to `crates/flowforge-core/src/error.rs`:
   ```rust
   #[error("category: {0}")]
   Category(String),
   ```

## Updating `flowforge init`

The init command in `crates/flowforge-cli/src/commands/init.rs` sets up new projects. When adding features that need initialization:

1. **New directories**: add `std::fs::create_dir_all()` call in `init_project()`
2. **New hook events**: add to `flowforge_hooks()` JSON builder
3. **Hook matchers**: PreToolUse must NOT have a matcher (guidance gates need all tools). Only add matchers if a hook genuinely only applies to specific tools.
4. **New settings.json fields**: add to `write_settings_json()`
5. **New CLAUDE.md sections**: add to `write_claude_md()`
6. **New MCP config**: modify `write_mcp_json()` if adding server args

## Checklist for Any Change

- [ ] `cargo build --workspace` compiles with 0 warnings
- [ ] `cargo test --workspace` all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` is clean
- [ ] `cargo fmt --all` is applied
- [ ] `README.md` is updated (if user-facing)
- [ ] `SETUP.md` is updated (if setup/config changes)
- [ ] `CLAUDE.md` is updated (if Claude needs to know about the change)
- [ ] MCP tool count test is updated (if tools added/removed)
- [ ] `init.rs` is updated (if new projects need the change)
- [ ] `.claude/settings.json` is consistent with `init.rs` template

## Key Files Reference

| File | Purpose |
|------|---------|
| `crates/flowforge-cli/src/main.rs` | CLI entry point, all command definitions |
| `crates/flowforge-cli/src/commands/init.rs` | Project initialization, hook wiring template |
| `crates/flowforge-cli/src/hooks/mod.rs` | Hook shared context (`HookContext`), `run_safe()`, `resolve_work_item_for_task()`, session-end learning (finalize routing outcomes, adaptive weights) |
| `crates/flowforge-cli/src/hooks/user_prompt_submit.rs` | Heaviest hook: routing, pattern/error/dependency injection, lazy embedder (OnceCell), prompt-length gate, immediate routing outcome recording, instant routing vector creation, auto-backfill of routing vectors |
| `crates/flowforge-cli/src/hooks/post_tool_use.rs` | Trajectory steps, edit tracking, Claude Tasks sync, **injection follow-through tracking** (5 types: routing, general, test suggestions, file dependencies, patterns), active learning routing weight updates |
| `crates/flowforge-cli/src/hooks/post_tool_use_failure.rs` | Error fingerprinting (`chars().take(200)` truncation), error context in trajectory steps, active learning routing weight penalties |
| `crates/flowforge-cli/src/hooks/pre_tool_use.rs` | Guidance gates, failure prevention with escalation (ASK at 2, DENY at `failure_deny_threshold`), work-gate |
| `crates/flowforge-cli/src/hooks/*.rs` | 13 hook handlers (one per file) |
| `crates/flowforge-core/src/types/*.rs` | All shared types, decomposed by domain (work, sessions, agents, guidance, patterns, collaboration) |
| `crates/flowforge-core/src/config.rs` | All config structs with defaults and cross-field validation; includes `failure_deny_threshold`, `promotion_min_usage`, `promotion_min_confidence` |
| `crates/flowforge-core/src/error.rs` | Error enum |
| `crates/flowforge-core/src/guidance/*.rs` | Guidance engine (5 gates), decomposed into engine + gates |
| `crates/flowforge-core/src/plugin.rs` | Plugin manifest loader |
| `crates/flowforge-core/src/plugin_exec.rs` | Plugin command execution |
| `crates/flowforge-core/src/work_tracking/*.rs` | WorkBackend trait, KanbusBackend, BeadsBackend, WorkDb trait, work-stealing, Claude Tasks sync, status validation |
| `crates/flowforge-core/src/trajectory.rs` | Trajectory types |
| `crates/flowforge-memory/src/db/mod.rs` | MemoryDb struct, `with_transaction()`, core DB methods |
| `crates/flowforge-memory/src/db/schema.rs` | SQLite schema, indexes, migrations |
| `crates/flowforge-memory/src/db/sessions.rs` | Session CRUD (transaction-wrapped cascade) |
| `crates/flowforge-memory/src/db/work_items.rs` | Work item CRUD, work-stealing, title-based lookup |
| `crates/flowforge-memory/src/db/routing.rs` | Routing weights: `record_routing_success`, `record_routing_failure` |
| `crates/flowforge-memory/src/db/adaptive_routing.rs` | Adaptive weight computation, `finalize_routing_outcomes`, routing outcome storage |
| `crates/flowforge-memory/src/db/effectiveness.rs` | Context injection tracking, `rate_injection`, pattern effectiveness scoring |
| `crates/flowforge-memory/src/db/file_dependencies.rs` | File co-edit tracking, `record_file_co_edit_pair` for real-time updates |
| `crates/flowforge-memory/src/db/error_recovery.rs` | Error fingerprints, resolutions, `auto_detect_resolutions` |
| `crates/flowforge-memory/src/db/vectors.rs` | HNSW vector storage, clustering, multi-source search |
| `crates/flowforge-memory/src/db/tests.rs` | DB test suite |
| `crates/flowforge-memory/src/patterns/mod.rs` | Pattern store with instant promotion on `record_feedback` |
| `crates/flowforge-memory/src/patterns/lifecycle.rs` | Promotion, demotion, decay, consolidation, deduplication |
| `crates/flowforge-memory/src/trajectory.rs` | Trajectory judge (judgment, distillation, consolidation) |
| `crates/flowforge-mcp/src/tools/*.rs` | MCP tool registry + dispatch, decomposed by category |
| `crates/flowforge-mcp/src/params.rs` | `ParamExt` trait with `require_str()` for MCP parameter validation |
| `crates/flowforge-mcp/src/server.rs` | JSON-RPC server, tool count test |
| `crates/flowforge-agents/src/registry.rs` | Agent loader (built-in + project + plugin) |
| `.claude/settings.json` | Live hook wiring (must match init.rs template) |
| `.mcp.json` | MCP server registration |

## Active Learning Architecture

FlowForge implements closed-loop learning that improves on every interaction:

1. **`UserPromptSubmit`** records a routing outcome (pending) + creates a routing vector + stores `active_routing:{session_id}` KV
2. **`PostToolUse`** reads the KV and calls `record_routing_success` on every successful tool call; checks injection follow-through (did Claude use the routing suggestion? run the suggested test? edit the suggested file? benefit from the injected pattern?)
3. **`PostToolUseFailure`** calls `record_routing_failure` on every failed tool call
4. **`SessionEnd`** finalizes pending routing outcomes to actual verdict, triggers adaptive weight computation if ≥5 outcomes exist
5. **Pattern feedback** from injection follow-through immediately promotes patterns that meet thresholds (`usage >= 1, confidence >= 0.5`)

This means every prompt teaches routing, every tool call reinforces or penalizes agents, and every confirmed pattern use promotes patterns — all within the same session.
