use serde_json::{json, Value};

use flowforge_memory::MemoryDb;

use crate::params::ParamExt;

use super::current_session_id;

pub fn claim(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let id = p.require_str("id")?;
    let session_id = current_session_id(db);
    let claimed = db.claim_work_item(id, &session_id)?;
    Ok(json!({"status": "ok", "claimed": claimed, "id": id}))
}

pub fn release(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let id = p.require_str("id")?;
    db.release_work_item(id)?;
    Ok(json!({"status": "ok", "id": id}))
}

pub fn steal(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let id = p.opt_str("id");
    let session_id = current_session_id(db);
    let target = match id {
        Some(id) => id.to_string(),
        None => {
            let items = db.get_stealable_items(1)?;
            items.first().map(|i| i.id.clone()).unwrap_or_default()
        }
    };
    if target.is_empty() {
        return Ok(json!({"status": "ok", "stolen": false, "id": ""}));
    }
    let stolen = db.steal_work_item(&target, &session_id)?;
    Ok(json!({"status": "ok", "stolen": stolen, "id": target}))
}

pub fn heartbeat(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let progress = p.opt_i64("progress").map(|v| v as i32);
    let id = p.opt_str("id");
    let session_id = current_session_id(db);
    let updated = db.update_heartbeat(&session_id)?;
    if let (Some(id), Some(progress)) = (id, progress) {
        db.update_progress(id, progress)?;
    }
    Ok(json!({"status": "ok", "items_updated": updated}))
}

pub fn stealable(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let limit = p.u64_or("limit", 10) as usize;
    let items = db.get_stealable_items(limit)?;
    let list: Vec<Value> = items
        .iter()
        .map(|i| {
            json!({
                "id": i.id,
                "title": i.title,
                "priority": i.priority,
                "progress": i.progress,
                "claimed_by": i.claimed_by,
            })
        })
        .collect();
    Ok(json!({"status": "ok", "items": list, "count": list.len()}))
}
