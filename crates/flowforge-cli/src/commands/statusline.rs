use colored::Colorize;
use flowforge_core::{AgentSessionStatus, FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;

pub fn run() -> Result<()> {
    // Read stdin (Claude Code pipes JSON context), but handle empty/missing gracefully
    let stdin_data: serde_json::Value = {
        let mut buf = String::new();
        match std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf) {
            Ok(_) if !buf.trim().is_empty() => {
                serde_json::from_str(&buf).unwrap_or(serde_json::json!({}))
            }
            _ => serde_json::json!({}),
        }
    };

    let model = stdin_data
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Try to load project info
    let project_name = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    // Try to get DB stats
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path()).ok();
    let db = config
        .as_ref()
        .and_then(|c| MemoryDb::open(&c.db_path()).ok());

    let mut parts = Vec::new();

    // Project name
    parts.push(format!("{} {}", "FF".bold().cyan(), project_name.bold()));

    // Model
    if !model.is_empty() {
        parts.push(model.dimmed().to_string());
    }

    // Session edits/commands
    if let Some(ref db) = db {
        if let Ok(Some(session)) = db.get_current_session() {
            parts.push(format!("{}/{}", session.edits, session.commands));
        }
    }

    // Agent sessions
    if let Some(ref db) = db {
        if let Ok(agents) = db.get_active_agent_sessions() {
            if !agents.is_empty() {
                let active = agents
                    .iter()
                    .filter(|a| a.status == AgentSessionStatus::Active)
                    .count();
                let idle = agents
                    .iter()
                    .filter(|a| a.status == AgentSessionStatus::Idle)
                    .count();
                let mut agent_str = format!("{} agents", agents.len());
                let mut details = Vec::new();
                if active > 0 {
                    details.push(format!("{} active", active));
                }
                if idle > 0 {
                    details.push(format!("{} idle", idle));
                }
                if !details.is_empty() {
                    agent_str = format!("{} ({})", agent_str, details.join(", "));
                }
                parts.push(agent_str.green().to_string());
            }
        }
    }

    // Pattern count
    if let Some(ref db) = db {
        let short = db.count_patterns_short().unwrap_or(0);
        let long = db.count_patterns_long().unwrap_or(0);
        let total = short + long;
        if total > 0 {
            parts.push(format!("{} patterns", total));
        }
    }

    // Memory count
    if let Some(ref db) = db {
        let count = db.count_kv().unwrap_or(0);
        if count > 0 {
            parts.push(format!("{} memories", count));
        }
    }

    print!("{}", parts.join(" | "));

    Ok(())
}
