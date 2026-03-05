use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use flowforge_core::FlowForgeConfig;
use flowforge_memory::MemoryDb;

use crate::params::ParamExt;

use super::current_session_id;

pub fn rules(config: &FlowForgeConfig) -> flowforge_core::Result<Value> {
    let g = &config.guidance;
    let mut rules = vec![];
    if g.destructive_ops_gate {
        rules.push(json!({"name": "destructive_ops", "enabled": true, "description": "Block dangerous commands"}));
    }
    if g.file_scope_gate {
        rules.push(json!({"name": "file_scope", "enabled": true, "description": "Block writes to protected paths"}));
    }
    if g.diff_size_gate {
        rules.push(json!({"name": "diff_size", "enabled": true, "max_lines": g.max_diff_lines, "description": "Ask for large diffs"}));
    }
    if g.secrets_gate {
        rules.push(json!({"name": "secrets", "enabled": true, "description": "Detect API keys and secrets"}));
    }
    for rule in &g.custom_rules {
        rules.push(json!({
            "name": rule.id,
            "enabled": rule.enabled,
            "pattern": rule.pattern,
            "action": format!("{}", rule.action),
            "scope": format!("{}", rule.scope),
            "description": rule.description
        }));
    }
    Ok(json!({
        "status": "ok",
        "gates": rules,
        "trust_config": {
            "initial": g.trust_initial_score,
            "ask_threshold": g.trust_ask_threshold,
            "decay_per_hour": g.trust_decay_per_hour
        }
    }))
}

pub fn trust(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let session_id = p.opt_str("session_id");
    let sid = match session_id {
        Some(s) => s.to_string(),
        None => current_session_id(db),
    };
    match db.get_trust_score(&sid)? {
        Some(t) => Ok(json!({
            "status": "ok",
            "session_id": sid,
            "score": t.score,
            "total_checks": t.total_checks,
            "denials": t.denials,
            "asks": t.asks,
            "allows": t.allows
        })),
        None => Ok(json!({
            "status": "ok",
            "session_id": sid,
            "score": null,
            "message": "no trust score found"
        })),
    }
}

pub fn audit(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let session_id = p.opt_str("session_id");
    let limit = p.u64_or("limit", 20) as usize;
    let sid = match session_id {
        Some(s) => s.to_string(),
        None => current_session_id(db),
    };
    let decisions = db.get_gate_decisions(&sid, limit)?;
    let entries: Vec<Value> = decisions
        .iter()
        .map(|d| {
            json!({
                "gate_name": d.gate_name,
                "tool_name": d.tool_name,
                "action": format!("{}", d.action),
                "reason": d.reason,
                "risk_level": format!("{}", d.risk_level),
                "trust_before": d.trust_before,
                "trust_after": d.trust_after,
                "timestamp": d.timestamp.to_rfc3339()
            })
        })
        .collect();
    Ok(json!({"status": "ok", "count": entries.len(), "entries": entries}))
}

pub fn verify(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let session_id = p.opt_str("session_id");
    let sid = match session_id {
        Some(s) => s.to_string(),
        None => current_session_id(db),
    };
    let decisions = db.get_gate_decisions_asc(&sid, 10000)?;
    if decisions.is_empty() {
        return Ok(
            json!({"status": "ok", "valid": 0, "invalid": 0, "total": 0, "message": "no audit entries"}),
        );
    }
    let mut prev_hash = String::new();
    let mut valid = 0u32;
    let mut invalid = 0u32;
    for d in &decisions {
        let expected_input = format!("{}{}{}{}", d.session_id, d.tool_name, d.reason, prev_hash);
        let expected_hash = format!("{:x}", Sha256::digest(expected_input.as_bytes()));
        if d.hash == expected_hash && d.prev_hash == prev_hash {
            valid += 1;
        } else {
            invalid += 1;
        }
        prev_hash = d.hash.clone();
    }
    let status = if invalid == 0 { "ok" } else { "broken" };
    Ok(json!({
        "status": status,
        "valid": valid,
        "invalid": invalid,
        "total": valid + invalid
    }))
}
