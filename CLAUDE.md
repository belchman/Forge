## [FlowForge] Agent Orchestration — Operational Reference

Rust workspace: 6 crates, 60 agents, 68 MCP tools, 13 hooks.
GitHub: https://github.com/belchman/flow-forge

---

### 1. Behavioral Rules

- Do what's asked — nothing more, nothing less.
- NEVER create files unless necessary; prefer editing existing files.
- ALWAYS read a file before editing it.
- Never commit secrets, credentials, or `.env` files.
- Always follow FlowForge routing directives injected via hooks.
- Every non-trivial task MUST have a kanbus work item (see §9).
- Never add co-author to git commits.
- Never use shell redirection (`>`) to truncate files — stdin pipe from Claude Code will hang. Use `truncate -s 0` instead.

---

### 2. Project Architecture

#### Crate Map

| Crate | Path | Purpose |
|-------|------|---------|
| `flowforge-cli` | `crates/flowforge-cli/` | CLI entry point (100+ commands), 13 hooks |
| `flowforge-core` | `crates/flowforge-core/` | Types, config, work tracking, error types |
| `flowforge-memory` | `crates/flowforge-memory/` | SQLite DB, HNSW vector search, schema v11 |
| `flowforge-agents` | `crates/flowforge-agents/` | Agent registry, 6-signal router |
| `flowforge-mcp` | `crates/flowforge-mcp/` | MCP server, 68 JSON-RPC 2.0 tools |
| `flowforge-tmux` | `crates/flowforge-tmux/` | tmux monitor integration |

#### Key Paths

| Path | Purpose |
|------|---------|
| `~/.cargo/bin/flowforge` | Installed binary |
| `.flowforge/config.toml` | Project config |
| `.flowforge/flowforge.db` | SQLite database |
| `.flowforge/hook-errors.log` | Hook error log |
| `.flowforge/plugins/` | Plugin directory (TOML manifests) |
| `agents/**/*.md` | 60 agent definitions |
| `.claude/settings.json` | Hooks config |
| `.mcp.json` | MCP server config |
| `crates/flowforge-cli/tests/cli.rs` | CLI integration tests |
| `CONTRIBUTING.md` | Dev guide (what to update when changing things) |

---

### 3. Concurrency & Batching

- Batch all `Task` tool spawns in ONE message for parallel execution.
- Batch all independent file reads/writes in ONE message.
- Maximum 6-8 agents per team for optimal coordination.
- Use hierarchical topology + raft consensus for anti-drift in swarms.
- When tool calls have no dependencies, always call them in parallel.

---

### 4. Task Complexity Heuristics

**Use agent teams when:**
- Touching 3+ files across modules
- New feature with tests + docs
- Cross-crate refactoring
- API changes with downstream consumers

**Skip teams for:**
- Single file edits, 1-2 line fixes
- Documentation-only changes
- Config changes
- Quick questions or research

---

### 5. Agent Routing (IMPORTANT)

FlowForge injects `[FlowForge Routing]` directives into your context via hooks. **Follow them.**

#### Routing Tiers

| Tier | Confidence | Action |
|------|-----------|--------|
| **DISPATCH** | ≥70% | Always use Task tool with specified `subagent_type`. Best match. |
| **Use** | ≥50% | Delegate via Task tool unless trivial (single-line fix, quick answer). |
| **Consider** | <50% | Use judgment — delegate if specialization helps. |

- Include the agent name in the Task tool's `name` parameter for tracking.
- Pass `subagent_type` directly from the directive to the Task tool.

#### 6-Signal Scoring

| Signal | Source | Weight (default) |
|--------|--------|-----------------|
| Pattern | Regex pattern matching | configurable |
| Capability | Keyword overlap | configurable |
| Learned | Historical routing weights | configurable |
| Priority | Agent priority (Critical/High/Normal/Low) | configurable |
| Context | File extension, tool usage, continuity | configurable |
| Semantic | Embedding cosine similarity (HashEmbedder 128-dim) | configurable |

Weights auto-normalize if sum drifts >0.01 from 1.0. Confidence sharpening via sigmoid (k=8.0).

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge route "<task>"` | Get best agent match with confidence score |
| `flowforge agent list` | List all 60 agents |
| `flowforge agent info <name>` | Agent details (capabilities, patterns, priority) |
| `flowforge agent search <query>` | Search agents by keyword |

#### MCP Tools
`agents_list`, `agents_route`, `agents_info`

---

### 6. Agent Teams

- For multi-file changes, use agent teams (`TeamCreate` + `Task`).
- FlowForge routes tasks to specialized agents and provides context.

#### Best Practices
- Create all tasks (`TaskCreate`) before spawning agents.
- Spawn agents in background when possible (`run_in_background: true`).
- Use descriptive names for agents matching their role.
- Maximum 6-8 agents per team.
- Use hierarchical topology + raft consensus for anti-drift.
- Graceful shutdown: send `shutdown_request` via `SendMessage` when done.

---

### 7. Dual Memory System

FlowForge uses BOTH a fast Rust-based memory system AND Claude's native auto-memory.

#### FlowForge Memory (fast, semantic, searchable)

SQLite + HNSW vector search with semantic embeddings (AllMiniLM-L6-v2) and DBSCAN topic clustering.

| Command | Purpose |
|---------|---------|
| `flowforge memory get <key> [--namespace <ns>]` | Retrieve value |
| `flowforge memory set <key> <value> [--namespace <ns>]` | Store key-value pair |
| `flowforge memory delete <key> [--namespace <ns>]` | Remove entry |
| `flowforge memory list [--namespace <ns>]` | List all entries |
| `flowforge memory search <query> [--limit N]` | Semantic vector search |

**MCP tools:** `memory_get`, `memory_set`, `memory_search`, `memory_delete`, `memory_list`, `memory_import`

#### Claude's Auto-Memory (cross-session, natural language)

Claude's built-in `MEMORY.md` and topic files for high-level insights. Best for architectural decisions, user preferences, project conventions.

#### When to Use Which

| Use FlowForge memory for | Use Claude memory for |
|--------------------------|----------------------|
| Routing weights, patterns, metrics | Design decisions, workflow preferences |
| Structured data, fast lookup | Project philosophy, nuanced context |
| Session history, edit records | Architectural conventions |

Use BOTH for critical knowledge — redundancy improves recall.

---

### 8. Pattern Learning & Trajectories

Records execution paths (tool sequences), judges outcomes, and distills reusable strategy patterns.

#### Lifecycle
```
observation → store → search/feedback → promotion (short→long) → clustering → injection
```

Patterns ranked by `similarity * (0.5 + 0.5 * effectiveness_score)` with token budget cap (default 2000).

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge learn store <content> [--category <cat>]` | Store a pattern (categories: general, code_style, error_fix, etc.) |
| `flowforge learn search <query> [--limit N]` | Search patterns semantically |
| `flowforge learn stats [--json]` | Pattern/trajectory statistics |
| `flowforge learn trajectories [--session <id>] [--status <s>] [--limit N]` | List trajectories (status: recording, completed, failed, judged) |
| `flowforge learn trajectory <id>` | Show trajectory detail |
| `flowforge learn judge <id>` | Manually judge a trajectory |
| `flowforge learn patterns [--mine] [--min_occurrences N]` | List/mine failure patterns |
| `flowforge learn clusters` | Show DBSCAN topic clusters |
| `flowforge learn tune-clusters` | Auto-tune DBSCAN parameters |
| `flowforge learn dependencies [--file <path>] [--limit N]` | File co-edit dependency graph |
| `flowforge learn download-model` | Download AllMiniLM-L6-v2 semantic model |

#### MCP Tools
`learning_store`, `learning_search`, `learning_feedback`, `learning_stats`, `learning_clusters`, `trajectory_list`, `trajectory_get`, `trajectory_judge`, `failure_patterns`, `similar_trajectories`, `task_decomposition`, `batching_insights`, `file_dependencies`

#### What's Automatic
- Trajectory recording (every tool call in session)
- Trajectory judging at session end (success ratio + work item completion)
- Pattern promotion (short-term → long-term based on effectiveness)
- Cluster updates on learning operations
- Context injection of relevant patterns on `UserPromptSubmit`
- Self-tuning injection threshold (adjusts `min_injection_similarity` from effectiveness data)

#### What You Do Manually
- `flowforge learn store` — Capture important patterns explicitly
- `flowforge learn judge` — Override automatic trajectory judgments

---

### 9. Work Tracking (MANDATORY)

**Every non-trivial task MUST have a kanbus work item.** This is not optional. FlowForge uses kanbus as the persistent backend — work items created here are stored as kanbus issues with full comment history, status tracking, and cross-session visibility.

#### When to Create Work Items
- **BEFORE starting** any feature, fix, refactor, or multi-step task
- One work item per logical unit of work (not per file or per step)
- Use `TaskCreate` for in-session step tracking (ephemeral). Use `flowforge work create` for persistent kanbus tracking (durable). **Always do both.**

#### Work Item Lifecycle
```
create → claim → update(in_progress) → [comment progress] → close
```

| Step | Command | When |
|------|---------|------|
| **Create** | `flowforge work create "<title>" --type task` | Before starting work. Add `--description "<details>"` for context. |
| **Claim** | `flowforge work claim <id>` | When you begin working on it. Sets `claimed_by` to current session. |
| **Start** | `flowforge work update <id> --status in_progress` | Immediately after claiming. Required before close. |
| **Comment** | `flowforge work comment <id> "<text>"` | Log progress, decisions, blockers, or completion notes. **Syncs directly to kanbus issue comments.** Use liberally. |
| **Close** | `flowforge work close <id>` | When work is complete. Must be in `in_progress` status first. |
| **Release** | `flowforge work release <id>` | If you can't finish — releases claim so others can steal. |

#### Valid Status Transitions
```
pending → in_progress → completed
pending → blocked → in_progress → completed
in_progress → pending (revert)
completed → pending (reopen)
```
**Invalid:** `pending → completed` (must go through `in_progress` first).

#### All CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge work create "<title>" --type task [--description "<desc>"] [--priority N] [--parent <id>]` | Create persistent work item |
| `flowforge work list [--status pending\|in_progress\|completed\|blocked] [--type <type>] [--json]` | List work items |
| `flowforge work get <id> [--json]` | Full details for a work item |
| `flowforge work update <id> --status <status>` | Change status |
| `flowforge work close <id>` | Complete a work item |
| `flowforge work delete <id>` | Delete a work item |
| `flowforge work comment <id> "<text>"` | Add comment (syncs to kanbus) |
| `flowforge work claim <id>` | Claim for current session |
| `flowforge work release <id>` | Release claim |
| `flowforge work steal [<id>]` | Steal an abandoned item |
| `flowforge work stealable` | List abandoned work items |
| `flowforge work heartbeat [<id>]` | Keep-alive for claimed items |
| `flowforge work load` | Work distribution across agents |
| `flowforge work sync` | Bi-directional sync with kanbus |
| `flowforge work status [--json]` | Summary counts |
| `flowforge work log [--limit N] [--since <date>]` | Audit trail |

**Types:** task, epic, bug, story, sub-task. **Priority:** 0=critical, 1=high, 2=normal (default), 3=low. ID supports prefix matching.

#### MCP Tools
`work_create`, `work_list`, `work_update`, `work_close`, `work_comment`, `work_log`, `work_sync`, `work_load`, `work_claim`, `work_release`, `work_steal`, `work_stealable`, `work_heartbeat`, `work_status`

#### What Gets Auto-Tracked (no manual call needed)
- Status change comments automatically added to kanbus on every `work update`/`work close`
- Heartbeat updated on every `pre_tool_use` hook (prevents work-stealing)
- Work items synced from kanbus on session start, pushed on session end
- Stale items auto-detected and made stealable after 30min without heartbeat
- Routing outcomes, trajectory verdicts, and session learning all recorded automatically

#### What You MUST Do Manually
1. `flowforge work create` — Create the work item BEFORE starting
2. `flowforge work update <id> --status in_progress` — Mark it active
3. `flowforge work comment <id> "<progress notes>"` — Document what you did, decisions made, files changed
4. `flowforge work close <id>` — Close when done
5. Comment on blockers, approach changes, or anything the user should know

---

### 10. Session Management

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge session current [--json]` | Active session info |
| `flowforge session list [--limit N]` | Recent sessions |
| `flowforge session metrics` | Session totals/averages |
| `flowforge session agents [--session_id <id>]` | Agent sessions (recursive CTE tree) |
| `flowforge session history [--session_id <id>] [--limit N] [--offset N]` | Conversation messages |
| `flowforge session ingest <path> [--session_id <id>]` | Ingest JSONL transcript |
| `flowforge session checkpoint <name> [--session_id <id>] [--description <text>]` | Create named checkpoint |
| `flowforge session checkpoints [--session_id <id>]` | List checkpoints |
| `flowforge session fork [--session_id <id>] [--checkpoint <name>] [--at_index N] [--reason <text>]` | Fork conversation |
| `flowforge session forks [--session_id <id>]` | List forks |
| `flowforge session hook-timing [--session_id <id>] [--json]` | Per-hook performance table (ms, calls, errors) |

#### MCP Tools
`session_status`, `session_metrics`, `session_history`, `session_agents`, `session_cost`, `conversation_history`, `conversation_search`, `conversation_ingest`, `checkpoint_create`, `checkpoint_list`, `checkpoint_get`, `session_fork`, `session_forks`, `session_lineage`

#### What's Automatic
- Session created on `SessionStart` hook
- Session continuity context injected (previous session summary)
- Anti-drift detection (cosine similarity <0.25 after 20+ commands)
- Auto-checkpoint before risky operations (git stash create)
- Tool success rate tracking per agent
- Hook timing metrics recorded per invocation
- Retention pruning (90 days default, runs at `SessionEnd`)

---

### 11. Error Recovery Intelligence

Tracks error fingerprints (normalize → SHA-256), matches known resolutions, and auto-injects fix suggestions when errors recur.

#### Lifecycle
```
error → fingerprint → match resolution → auto-inject on recurrence
```

Failure loop detection: warn after 1st recurrence, ask after 2nd.

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge error list [--limit N]` | Known error patterns |
| `flowforge error find <error_text>` | Find matching resolutions |
| `flowforge error stats` | Error recovery statistics |

#### MCP Tools
`error_list`, `error_find`, `error_stats`, `recovery_strategies`

#### What's Automatic
- Error fingerprinting on `PostToolUseFailure` hook
- Resolution matching and injection on `UserPromptSubmit` hook
- Failure pattern mining during `learn patterns --mine`

---

### 12. Guidance Control Plane

Enforces configurable safety rules on ALL tool uses via the `PreToolUse` hook. Can block tool execution (deny/ask).

#### 5 Built-in Gates

| Gate | Purpose |
|------|---------|
| Destructive ops | Blocks `rm -rf`, `git reset --hard`, `DROP TABLE`, etc. |
| Secrets detection | Scans string values (≤10KB) for API keys, tokens, passwords |
| File scope | Restricts writes to project directory (prefix match) |
| Custom rules | User-defined rules in config |
| Diff size | Warns on large diffs |

#### Trust Scoring

| Event | Score Change |
|-------|-------------|
| Initial | 0.8 |
| Tool denied | -0.1 |
| Tool asked (user prompted) | -0.02 |
| Tool allowed | +0.01 |

Trust decays over time. Auto-promotion at configurable thresholds.

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge guidance rules` | Active gates and trust config |
| `flowforge guidance trust [--session <id>]` | Session trust score |
| `flowforge guidance audit [--session <id>] [--limit N]` | Gate decision trail (SHA-256 hash chain) |
| `flowforge guidance verify [--session <id>]` | Verify hash chain integrity |

#### MCP Tools
`guidance_rules`, `guidance_trust`, `guidance_audit`, `guidance_verify`

---

### 13. Mailbox / Co-Agent Communication

Message routing through work item coordination hubs for multi-agent collaboration.

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge mailbox send --work_item <id> --from <agent> [--to <agent>] <message>` | Send message (broadcast if `--to` omitted) |
| `flowforge mailbox read [--session_id <id>]` | Read unread messages |
| `flowforge mailbox history <work_item_id> [--limit N]` | Message history for work item |
| `flowforge mailbox agents <work_item_id>` | List agents on work item |

#### MCP Tools
`mailbox_send`, `mailbox_read`, `mailbox_history`, `mailbox_agents`

---

### 14. Plugin SDK

Extend FlowForge with custom tools, hooks, and agents without recompilation. Plugins live in `.flowforge/plugins/<name>/` with a `plugin.toml` manifest.

- Plugin tools execute shell commands with JSON stdin/stdout.
- Plugin hooks run in priority order during `PreToolUse` and other events.
- Plugin agents load as markdown files with `AgentSource::Plugin`.

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge plugin list` | List installed plugins |
| `flowforge plugin info <name>` | Plugin details |
| `flowforge plugin enable <name>` | Enable a plugin |
| `flowforge plugin disable <name>` | Disable a plugin |

#### MCP Tools
`plugin_list`, `plugin_info`

---

### 15. Tool Metrics & Intelligence

Tracks tool success rates per agent, identifies best agents for specific tools, and estimates session cost.

#### MCP Tools
`tool_metrics`, `tool_best_agents`, `session_cost`

#### What's Automatic
- Tool outcomes recorded on `PostToolUse` / `PostToolUseFailure` hooks
- Success rate aggregation per agent per tool
- Session cost metrics accumulated during session

---

### 16. Team Monitoring (tmux)

Real-time team monitoring dashboard. Updates automatically via hooks.

#### CLI Commands

| Command | Purpose |
|---------|---------|
| `flowforge tmux start` | Start tmux monitor pane |
| `flowforge tmux update` | Refresh display |
| `flowforge tmux stop` | Stop monitor |
| `flowforge tmux status` | Current team state |

#### MCP Tools
`team_status`, `team_log`

---

### 17. Hooks Reference

All 13 hooks with trigger event, timeout, and key behavior.

| Hook | Trigger | Timeout | Key Actions | Output |
|------|---------|---------|-------------|--------|
| `SessionStart` | Session begins | 10s | Create session, sync kanbus, continuity context | Context |
| `UserPromptSubmit` | Each user prompt | 5s | Routing, patterns, work-gate, error recovery, anti-drift | Context |
| `PreToolUse` | Before tool exec | 3s | Guidance gates, work-gate, heartbeat, failure prevention | Deny/Ask/Allow |
| `PostToolUse` | After tool exec | 3s | Record edits, trajectory steps, sync Claude tasks | None |
| `PostToolUseFailure` | Tool fails | 3s | Record error fingerprint, failure pattern | None |
| `PreCompact` | Before context compaction | — | Consolidate patterns, preservation guidance | Context |
| `SubagentStart` | Agent spawns | — | Create agent session, inject agent context | Context |
| `SubagentStop` | Agent finishes | — | End agent session, rollup stats, extract patterns | None |
| `TaskCompleted` | Task marked done | — | Update routing weights, resolve work item | None |
| `TeammateIdle` | Teammate goes idle | — | Mark idle, detect stale work | None |
| `SessionEnd` | Session ends | — | End session, judge trajectory, routing feedback, retention prune | None |
| `Stop` | Process stop signal | — | End session, run learning (lighter than SessionEnd) | None |
| `Notification` | Notification event | 3s | No-op | None |

#### Key Facts
- `UserPromptSubmit` is **exempt** from `FLOWFORGE_HOOKS_DISABLED` kill-switch (work-tracking always enforced).
- `PreToolUse` can **block** tool execution (deny/ask).
- Hooks with "Context" output inject content into Claude's context window.
- `SessionEnd` runs retention pruning (90 days default).

---

### 18. Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `FLOWFORGE_HOOKS_DISABLED=1` | Disable all hooks except `UserPromptSubmit` work-gate | unset |
| `FLOWFORGE_NO_WORK_GATE` | Disable work-item enforcement gate | unset |

---

### 19. Configuration & Building

#### Config Management

| Command | Purpose |
|---------|---------|
| `flowforge config show` | Print resolved config |
| `flowforge config get <key>` | Get value by dot-path (e.g., `patterns.short_term_max`) |
| `flowforge config set <key> <value>` | Set config value |
| `flowforge status [--json]` | Unified project dashboard |
| `flowforge test-hooks [--event <e>] [--verbose]` | Validate hook wiring |
| `flowforge init [--project] [--global]` | Initialize project/global config |

#### CLI vs MCP Delineation

| Interface | Audience | Format | When to Use |
|-----------|----------|--------|-------------|
| CLI (`flowforge`) | Human-facing | Colored terminal output | Terminal commands, scripts, debugging |
| MCP tools | Machine-facing | JSON in/out via JSON-RPC 2.0 | Claude Code via `.mcp.json`, programmatic access |

Same underlying DB and functions — CLI and MCP are two interfaces to the same system. `flowforge mcp serve` starts the MCP server over stdio.

#### Building & Installing

```bash
cargo build --release && rm -f ~/.cargo/bin/flowforge && cp target/release/flowforge ~/.cargo/bin/flowforge
```

**Critical:** Always `rm` the old binary before copying. macOS caches in-place overwrites, causing the stale binary to hang indefinitely. All hooks will silently fail/timeout if this happens. Or use `./setup.sh` which handles this automatically.

#### Key Config Sections

| Section | Key Settings |
|---------|-------------|
| `[hooks]` | `inject_agent_body = false` (1-line summary vs full body, ~460 tokens saved) |
| `[patterns]` | `min_injection_similarity = 0.55`, `context_budget = 2000`, `short_term_max` |
| `[guidance]` | `enabled`, trust thresholds, gate configs |
| `[routing]` | 6 signal weights, `confidence_sharpening` |
| `[memory]` | `retention_days = 90` |

---

### 20. Slash Commands

| Command | Purpose |
|---------|---------|
| `/status` | Unified project dashboard (work, learning, trust, agents) |
| `/test-hooks` | Run hook tests with diagnostics |
| `/setup` | Project initialization wizard (build, install, init, verify) |

---

### 21. Complete MCP Tool Reference (68 tools)

| Category | Tools |
|----------|-------|
| **Memory** (6) | `memory_get`, `memory_set`, `memory_search`, `memory_delete`, `memory_list`, `memory_import` |
| **Learning** (5) | `learning_store`, `learning_search`, `learning_feedback`, `learning_stats`, `learning_clusters` |
| **Agents** (3) | `agents_list`, `agents_route`, `agents_info` |
| **Session** (4) | `session_status`, `session_metrics`, `session_history`, `session_agents` |
| **Conversation** (6) | `conversation_history`, `conversation_search`, `conversation_ingest`, `checkpoint_create`, `checkpoint_list`, `checkpoint_get` |
| **Forks** (3) | `session_fork`, `session_forks`, `session_lineage` |
| **Work** (14) | `work_create`, `work_list`, `work_update`, `work_close`, `work_comment`, `work_log`, `work_sync`, `work_load`, `work_claim`, `work_release`, `work_steal`, `work_stealable`, `work_heartbeat`, `work_status` |
| **Mailbox** (4) | `mailbox_send`, `mailbox_read`, `mailbox_history`, `mailbox_agents` |
| **Guidance** (4) | `guidance_rules`, `guidance_trust`, `guidance_audit`, `guidance_verify` |
| **Plugins** (2) | `plugin_list`, `plugin_info` |
| **Trajectories** (3) | `trajectory_list`, `trajectory_get`, `trajectory_judge` |
| **Intelligence** (5) | `failure_patterns`, `similar_trajectories`, `task_decomposition`, `batching_insights`, `file_dependencies` |
| **Error Recovery** (4) | `error_list`, `error_find`, `error_stats`, `recovery_strategies` |
| **Tool Metrics** (3) | `tool_metrics`, `tool_best_agents`, `session_cost` |
| **Team** (2) | `team_status`, `team_log` |
