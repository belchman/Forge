use serde_json::{json, Value};

use flowforge_core::config::FlowForgeConfig;
use flowforge_memory::MemoryDb;

use crate::params::ParamExt;

/// List known error fingerprints with occurrence counts.
pub fn list(db: &MemoryDb, p: &Value) -> flowforge_core::Result<Value> {
    let limit = p.u64_or("limit", 20) as usize;
    let fingerprints = db.list_error_fingerprints(limit)?;

    let entries: Vec<Value> = fingerprints
        .iter()
        .map(|fp| {
            json!({
                "id": fp.id,
                "category": fp.category.to_string(),
                "tool_name": fp.tool_name,
                "error_preview": fp.error_preview,
                "first_seen": fp.first_seen.to_rfc3339(),
                "last_seen": fp.last_seen.to_rfc3339(),
                "occurrence_count": fp.occurrence_count,
            })
        })
        .collect();

    Ok(json!({"status": "ok", "errors": entries, "count": entries.len()}))
}

/// Find resolutions for an error (by text or fingerprint ID).
/// Falls back to semantic vector search when no exact fingerprint match is found.
pub fn find(db: &MemoryDb, config: &FlowForgeConfig, p: &Value) -> flowforge_core::Result<Value> {
    let limit = p.u64_or("limit", 5) as usize;

    // Accept either error_text or fingerprint_id
    if let Some(error_text) = p.opt_str("error_text") {
        match db.find_error_resolutions(error_text, limit)? {
            Some((fp, resolutions)) => {
                let res: Vec<Value> = resolutions
                    .iter()
                    .map(|r| {
                        json!({
                            "id": r.id,
                            "summary": r.resolution_summary,
                            "tool_sequence": r.tool_sequence,
                            "files_changed": r.files_changed,
                            "confidence": r.confidence(),
                            "success_count": r.success_count,
                            "failure_count": r.failure_count,
                        })
                    })
                    .collect();
                Ok(json!({
                    "status": "ok",
                    "found": true,
                    "match_type": "exact",
                    "fingerprint": {
                        "id": fp.id,
                        "category": fp.category.to_string(),
                        "occurrence_count": fp.occurrence_count,
                    },
                    "resolutions": res,
                }))
            }
            None => {
                // No exact fingerprint match — try semantic search
                if config.vectors.embed_errors {
                    let embedder =
                        flowforge_memory::default_embedder(&config.patterns);
                    let query_vec = embedder.embed(error_text);
                    let semantic_results =
                        db.find_error_resolutions_semantic(&query_vec, limit)?;

                    if !semantic_results.is_empty() {
                        let entries: Vec<Value> = semantic_results
                            .iter()
                            .map(|(fp, resolutions)| {
                                let res: Vec<Value> = resolutions
                                    .iter()
                                    .map(|r| {
                                        json!({
                                            "id": r.id,
                                            "summary": r.resolution_summary,
                                            "tool_sequence": r.tool_sequence,
                                            "files_changed": r.files_changed,
                                            "confidence": r.confidence(),
                                            "success_count": r.success_count,
                                            "failure_count": r.failure_count,
                                        })
                                    })
                                    .collect();
                                json!({
                                    "fingerprint": {
                                        "id": fp.id,
                                        "category": fp.category.to_string(),
                                        "error_preview": fp.error_preview,
                                        "occurrence_count": fp.occurrence_count,
                                    },
                                    "resolutions": res,
                                })
                            })
                            .collect();
                        return Ok(json!({
                            "status": "ok",
                            "found": true,
                            "match_type": "semantic",
                            "similar_errors": entries,
                        }));
                    }
                }
                Ok(json!({"status": "ok", "found": false, "resolutions": []}))
            }
        }
    } else if let Some(fingerprint_id) = p.opt_str("fingerprint_id") {
        let resolutions = db.get_resolutions_for_fingerprint(fingerprint_id, limit)?;
        let res: Vec<Value> = resolutions
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "summary": r.resolution_summary,
                    "tool_sequence": r.tool_sequence,
                    "files_changed": r.files_changed,
                    "confidence": r.confidence(),
                    "success_count": r.success_count,
                    "failure_count": r.failure_count,
                })
            })
            .collect();
        Ok(json!({"status": "ok", "found": !resolutions.is_empty(), "resolutions": res}))
    } else {
        Err(flowforge_core::Error::InvalidInput(
            "Either 'error_text' or 'fingerprint_id' is required".into(),
        ))
    }
}

/// Get error recovery statistics.
pub fn stats(db: &MemoryDb) -> flowforge_core::Result<Value> {
    let (fingerprints, resolutions, total_occurrences) = db.get_error_stats()?;
    Ok(json!({
        "status": "ok",
        "fingerprints": fingerprints,
        "resolutions": resolutions,
        "total_occurrences": total_occurrences,
    }))
}
