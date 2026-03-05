use serde_json::{json, Value};

use flowforge_memory::MemoryDb;

use crate::params::ParamExt;

pub fn send(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let work_item_id = p.str_or("work_item_id", "");
    let from_session_id = p.str_or("from_session_id", "");
    let from_agent_name = p.str_or("from_agent_name", "");
    let to_session_id = p.opt_str("to_session_id");
    let to_agent_name = p.opt_str("to_agent_name");
    let content = p.str_or("content", "");
    let message_type = p.str_or("message_type", "text");
    let priority = p.i64_or("priority", 2) as i32;
    let msg = flowforge_core::MailboxMessage {
        id: 0,
        work_item_id: work_item_id.to_string(),
        from_session_id: from_session_id.to_string(),
        from_agent_name: from_agent_name.to_string(),
        to_session_id: to_session_id.map(|s| s.to_string()),
        to_agent_name: to_agent_name.map(|s| s.to_string()),
        message_type: message_type.to_string(),
        content: content.to_string(),
        priority,
        read_at: None,
        created_at: chrono::Utc::now(),
        metadata: None,
    };
    let id = db.send_mailbox_message(&msg)?;
    Ok(json!({"status": "ok", "message_id": id}))
}

pub fn read(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let session_id = p.str_or("session_id", "");
    let msgs = db.get_unread_messages(session_id)?;
    let entries: Vec<Value> = msgs
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "from_agent_name": m.from_agent_name,
                "to_agent_name": m.to_agent_name,
                "message_type": m.message_type,
                "content": m.content,
                "priority": m.priority,
                "created_at": m.created_at.to_rfc3339(),
            })
        })
        .collect();
    let count = entries.len();
    let _ = db.mark_messages_read(session_id);
    Ok(json!({"status": "ok", "messages": entries, "count": count}))
}

pub fn history(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let work_item_id = p.str_or("work_item_id", "");
    let limit = p.u64_or("limit", 20) as usize;
    let msgs = db.get_mailbox_history(work_item_id, limit)?;
    let entries: Vec<Value> = msgs
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "from_agent_name": m.from_agent_name,
                "to_agent_name": m.to_agent_name,
                "message_type": m.message_type,
                "content": m.content,
                "priority": m.priority,
                "read_at": m.read_at.map(|t| t.to_rfc3339()),
                "created_at": m.created_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(json!({"status": "ok", "messages": entries}))
}

pub fn agents(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let work_item_id = p.str_or("work_item_id", "");
    let agent_list = db.get_agents_on_work_item(work_item_id)?;
    let entries: Vec<Value> = agent_list
        .iter()
        .map(|a| {
            json!({
                "agent_id": a.agent_id,
                "agent_type": a.agent_type,
                "status": a.status.to_string(),
                "started_at": a.started_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(json!({"status": "ok", "agents": entries, "count": entries.len()}))
}
