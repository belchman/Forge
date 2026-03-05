use serde_json::{json, Value};

use flowforge_memory::MemoryDb;

use crate::params::ParamExt;

pub fn status(db: &MemoryDb) -> flowforge_core::Result<Value> {
    match db.get_current_session()? {
        Some(session) => {
            let agents: Vec<Value> = db
                .get_agent_sessions(&session.id)
                .unwrap_or_default()
                .iter()
                .filter(|a| a.ended_at.is_none())
                .map(|a| {
                    json!({
                        "agent_id": a.agent_id,
                        "agent_type": a.agent_type,
                        "status": a.status.to_string(),
                    })
                })
                .collect();
            Ok(json!({
                "status": "ok",
                "session": {
                    "id": session.id,
                    "started_at": session.started_at.to_rfc3339(),
                    "cwd": session.cwd,
                    "edits": session.edits,
                    "commands": session.commands,
                    "summary": session.summary,
                },
                "agents": agents,
            }))
        }
        None => Ok(json!({"status": "ok", "session": null})),
    }
}

pub fn metrics(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let session_id = p.opt_str("session_id");
    let session = if let Some(id) = session_id {
        db.list_sessions(1000)
            .ok()
            .and_then(|sessions| sessions.into_iter().find(|s| s.id == id))
    } else {
        db.get_current_session().ok().flatten()
    };
    match session {
        Some(s) => Ok(json!({
            "status": "ok",
            "session_id": s.id,
            "edits": s.edits,
            "commands": s.commands,
        })),
        None => Ok(json!({"status": "ok", "session_id": session_id, "edits": 0, "commands": 0})),
    }
}

pub fn history(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let limit = p.u64_or("limit", 10) as usize;
    let sessions = db.list_sessions(limit)?;
    let entries: Vec<Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "started_at": s.started_at.to_rfc3339(),
                "ended_at": s.ended_at.map(|t| t.to_rfc3339()),
                "cwd": s.cwd,
                "edits": s.edits,
                "commands": s.commands,
                "summary": s.summary,
            })
        })
        .collect();
    Ok(json!({"status": "ok", "sessions": entries}))
}

pub fn agents(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let session_id = p.opt_str("session_id");
    let parent_id = if let Some(id) = session_id {
        id.to_string()
    } else {
        match db.get_current_session()? {
            Some(s) => s.id,
            None => return Ok(json!({"status": "ok", "agents": [], "count": 0})),
        }
    };
    let agent_list = db.get_agent_sessions(&parent_id)?;
    let entries: Vec<Value> = agent_list
        .iter()
        .map(|a| {
            let duration_seconds = a.ended_at.map(|end| (end - a.started_at).num_seconds());
            json!({
                "id": a.id,
                "agent_id": a.agent_id,
                "agent_type": a.agent_type,
                "status": a.status.to_string(),
                "started_at": a.started_at.to_rfc3339(),
                "ended_at": a.ended_at.map(|t| t.to_rfc3339()),
                "edits": a.edits,
                "commands": a.commands,
                "task_id": a.task_id,
                "duration_seconds": duration_seconds,
            })
        })
        .collect();
    Ok(json!({"status": "ok", "agents": entries, "count": entries.len()}))
}
