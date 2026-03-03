use flowforge_core::{FlowForgeConfig, Result};
use colored::Colorize;
use std::path::Path;

pub fn run(project: bool, global: bool) -> Result<()> {
    if !project && !global {
        // Default to project init
        return init_project();
    }
    if project {
        init_project()?;
    }
    if global {
        init_global()?;
    }
    Ok(())
}

fn init_project() -> Result<()> {
    let project_dir = FlowForgeConfig::project_dir();
    std::fs::create_dir_all(&project_dir)?;
    std::fs::create_dir_all(project_dir.join("agents"))?;

    // Create default config
    let config = FlowForgeConfig::default();
    config.save(&FlowForgeConfig::config_path())?;
    println!("{} Created {}", "✓".green(), FlowForgeConfig::config_path().display());

    // Create database
    let db_path = config.db_path();
    let _db = flowforge_memory::MemoryDb::open(&db_path)?;
    println!("{} Created {}", "✓".green(), db_path.display());

    // Create/update .claude/settings.json with hooks
    write_settings_json()?;
    println!("{} Updated .claude/settings.json with FlowForge hooks", "✓".green());

    // Create CLAUDE.md additions
    write_claude_md()?;
    println!("{} Created/updated CLAUDE.md with FlowForge instructions", "✓".green());

    println!("\n{}", "FlowForge initialized!".green().bold());
    println!("Start a new Claude Code session to activate hooks.");

    Ok(())
}

fn init_global() -> Result<()> {
    let global_dir = FlowForgeConfig::global_dir();
    std::fs::create_dir_all(&global_dir)?;
    std::fs::create_dir_all(global_dir.join("agents"))?;

    let config_path = global_dir.join("config.toml");
    if !config_path.exists() {
        let config = FlowForgeConfig::default();
        config.save(&config_path)?;
    }

    println!("{} Global FlowForge initialized at {}", "✓".green(), global_dir.display());
    Ok(())
}

fn write_settings_json() -> Result<()> {
    let settings_dir = Path::new(".claude");
    std::fs::create_dir_all(settings_dir)?;

    let settings_path = settings_dir.join("settings.json");

    let settings = serde_json::json!({
        "env": {
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
        },
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook pre-tool-use",
                        "timeout": 3000
                    }]
                }
            ],
            "PostToolUse": [
                {
                    "matcher": "Write|Edit|MultiEdit",
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook post-tool-use",
                        "timeout": 3000
                    }]
                }
            ],
            "UserPromptSubmit": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook user-prompt-submit",
                        "timeout": 5000
                    }]
                }
            ],
            "SessionStart": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook session-start"
                    }]
                }
            ],
            "SessionEnd": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook session-end"
                    }]
                }
            ],
            "Stop": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook stop"
                    }]
                }
            ],
            "PreCompact": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook pre-compact"
                    }]
                }
            ],
            "SubagentStart": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook subagent-start"
                    }]
                }
            ],
            "SubagentStop": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook subagent-stop"
                    }]
                }
            ],
            "TeammateIdle": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook teammate-idle"
                    }]
                }
            ],
            "TaskCompleted": [
                {
                    "hooks": [{
                        "type": "command",
                        "command": "flowforge hook task-completed"
                    }]
                }
            ]
        }
    });

    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, content)?;

    Ok(())
}

fn write_claude_md() -> Result<()> {
    let claude_md_path = Path::new("CLAUDE.md");
    let mut content = String::new();

    if claude_md_path.exists() {
        content = std::fs::read_to_string(claude_md_path)?;
        if content.contains("[FlowForge]") {
            return Ok(()); // Already has FlowForge section
        }
        content.push_str("\n\n");
    }

    content.push_str(r#"## [FlowForge] Agent Orchestration

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

### tmux Monitor
- Run `flowforge tmux start` for real-time team monitoring
- The monitor updates automatically via hooks

### Available Agents
- Run `flowforge agent list` to see all available agents
- Run `flowforge route "<task>"` to get agent suggestions
- Run `flowforge learn stats` to check learning progress
"#);

    std::fs::write(claude_md_path, content)?;
    Ok(())
}
