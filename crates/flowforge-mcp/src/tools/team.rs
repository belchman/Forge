use serde_json::{json, Value};

use flowforge_core::FlowForgeConfig;
use flowforge_memory::MemoryDb;
use flowforge_tmux::TmuxStateManager;

use crate::params::ParamExt;

pub fn status() -> Value {
    let mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    match mgr.load() {
        Ok(state) => {
            let members: Vec<Value> = state
                .members
                .iter()
                .map(|m| {
                    json!({
                        "agent_id": m.agent_id,
                        "agent_type": m.agent_type,
                        "status": format!("{:?}", m.status),
                        "current_task": m.current_task,
                        "updated_at": m.updated_at.to_rfc3339(),
                    })
                })
                .collect();

            let agent_sessions: Vec<Value> =
                if let Ok(config) = FlowForgeConfig::load(&FlowForgeConfig::config_path()) {
                    if let Ok(db) = MemoryDb::open(&config.db_path()) {
                        db.get_active_agent_sessions()
                            .unwrap_or_default()
                            .iter()
                            .map(|a| {
                                json!({
                                    "id": a.id,
                                    "agent_id": a.agent_id,
                                    "agent_type": a.agent_type,
                                    "status": a.status.to_string(),
                                    "started_at": a.started_at.to_rfc3339(),
                                    "edits": a.edits,
                                    "commands": a.commands,
                                })
                            })
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

            json!({
                "status": "ok",
                "team": state.team_name,
                "members": members,
                "agent_sessions": agent_sessions,
                "memory_count": state.memory_count,
                "pattern_count": state.pattern_count,
                "updated_at": state.updated_at.to_rfc3339(),
            })
        }
        Err(e) => json!({"status": "error", "message": format!("{e}")}),
    }
}

pub fn log(p: &Value) -> Value {
    let limit = p.u64_or("limit", 20) as usize;
    let mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    match mgr.load() {
        Ok(state) => {
            let events: Vec<&String> = state.recent_events.iter().take(limit).collect();
            json!({"status": "ok", "events": events})
        }
        Err(e) => json!({"status": "error", "message": format!("{e}")}),
    }
}
