use serde_json::{json, Value};

use flowforge_core::FlowForgeConfig;
use flowforge_memory::MemoryDb;

use crate::params::ParamExt;

pub fn list(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let session_id = p.opt_str("session_id");
    let status = p.opt_str("status");
    let limit = p.u64_or("limit", 20) as usize;
    let trajectories = db.list_trajectories(session_id, status, limit)?;
    let entries: Vec<Value> = trajectories
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "session_id": t.session_id,
                "status": format!("{}", t.status),
                "verdict": t.verdict.as_ref().map(|v| format!("{v}")),
                "confidence": t.confidence,
                "task_description": t.task_description,
                "started_at": t.started_at.to_rfc3339()
            })
        })
        .collect();
    Ok(json!({"status": "ok", "count": entries.len(), "trajectories": entries}))
}

pub fn get(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let id = p.require_str("id")?;
    let trajectory = db.get_trajectory(id)?;
    let steps = db.get_trajectory_steps(id)?;
    let ratio = db.trajectory_success_ratio(id)?;
    match trajectory {
        Some(t) => {
            let step_entries: Vec<Value> = steps
                .iter()
                .map(|s| {
                    json!({
                        "step_index": s.step_index,
                        "tool_name": s.tool_name,
                        "outcome": format!("{}", s.outcome),
                        "duration_ms": s.duration_ms,
                        "timestamp": s.timestamp.to_rfc3339()
                    })
                })
                .collect();
            Ok(json!({
                "status": "ok",
                "id": t.id,
                "session_id": t.session_id,
                "status_field": format!("{}", t.status),
                "verdict": t.verdict.as_ref().map(|v| format!("{v}")),
                "confidence": t.confidence,
                "task_description": t.task_description,
                "success_ratio": ratio,
                "steps": step_entries
            }))
        }
        None => Ok(json!({"status": "error", "message": "trajectory not found"})),
    }
}

pub fn judge(db: &MemoryDb, config: &FlowForgeConfig, p: &Value) -> flowforge_core::Result<Value> {
    let id = p.require_str("id")?;
    let judge = flowforge_memory::trajectory::TrajectoryJudge::new(db, &config.patterns);
    let result = judge.judge(id)?;
    Ok(json!({
        "status": "ok",
        "verdict": format!("{}", result.verdict),
        "confidence": result.confidence,
        "reason": result.reason
    }))
}
