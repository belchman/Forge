## [FlowForge] Agent Orchestration

This project uses FlowForge for intelligent agent orchestration.

### Agent Teams
- For multi-file changes, use agent teams (TeamCreate + Task)
- FlowForge will route tasks to specialized agents and provide context
- Maximum 6-8 agents per team for optimal coordination
- Use the anti-drift swarm pattern (hierarchical topology, raft consensus)

### Dual Memory System
FlowForge uses BOTH a fast Rust-based memory system AND Claude's native auto-memory:

**FlowForge Memory (fast, structured, searchable):**
- SQLite + HNSW vector search for sub-millisecond pattern retrieval
- Stores learned patterns, routing weights, session history, edit records
- Use `flowforge memory set <key> <value>` for project-specific knowledge
- Use `flowforge memory search <query>` to recall stored knowledge
- Use `flowforge learn store "<pattern>" --category <cat>` for reusable patterns
- Automatically learns from agent outcomes (routing weights, success rates)
- MCP tools available: `memory_get`, `memory_set`, `memory_search`, `learning_store`

**Claude's Auto-Memory (semantic, cross-session, natural language):**
- Claude's built-in MEMORY.md and topic files for high-level insights
- Best for architectural decisions, user preferences, project conventions
- Persists across all sessions automatically
- Natural language — good for nuanced context

**When to use which:**
- FlowForge memory: routing weights, patterns, metrics, structured data, fast lookup
- Claude memory: design decisions, workflow preferences, project philosophy
- Use BOTH for critical knowledge — redundancy improves recall

### Work Tracking
- FlowForge tracks all work items (epics, tasks, bugs) automatically via hooks
- Every task completion, agent assignment, and status change is logged
- Use `flowforge work status` to see active work
- Use `flowforge work create` to create tracked items
- Supported backends: Claude Tasks, Beads, Kanbus (auto-detected)
- MCP tools: `work_create`, `work_list`, `work_update`, `work_log`

### tmux Monitor
- Run `flowforge tmux start` for real-time team monitoring
- The monitor updates automatically via hooks

### Available Agents
- Run `flowforge agent list` to see all available agents
- Run `flowforge route "<task>"` to get agent suggestions
- Run `flowforge learn stats` to check learning progress
