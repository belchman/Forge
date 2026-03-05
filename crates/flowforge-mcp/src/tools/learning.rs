use serde_json::{json, Value};

use flowforge_core::FlowForgeConfig;
use flowforge_memory::{HnswCache, MemoryDb, PatternStore};

use crate::params::ParamExt;

pub fn store_cached(
    db: &MemoryDb,
    config: &FlowForgeConfig,
    p: &Value,
    cache: &HnswCache,
) -> flowforge_core::Result<Value> {
    let content = p.require_str("content")?;
    let category = p.require_str("category")?;
    let store = PatternStore::with_cache(db, &config.patterns, cache);
    let id = store.store_short_term(content, category)?;
    Ok(json!({"status": "ok", "pattern_id": id}))
}

pub fn search_cached(
    db: &MemoryDb,
    config: &FlowForgeConfig,
    p: &Value,
    cache: &HnswCache,
) -> flowforge_core::Result<Value> {
    let query = p.require_str("query")?;
    let limit = p.u64_or("limit", 10) as usize;
    let store = PatternStore::with_cache(db, &config.patterns, cache);
    let results = store.search_all_patterns(query, limit)?;
    let patterns: Vec<Value> = results
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "content": m.content,
                "category": m.category,
                "confidence": m.confidence,
                "usage_count": m.usage_count,
                "tier": format!("{:?}", m.tier),
                "similarity": m.similarity,
            })
        })
        .collect();
    Ok(json!({"status": "ok", "patterns": patterns}))
}

pub fn feedback_cached(
    db: &MemoryDb,
    config: &FlowForgeConfig,
    p: &Value,
    cache: &HnswCache,
) -> flowforge_core::Result<Value> {
    let pattern_id = p.require_str("pattern_id")?;
    let positive = p.bool_or("positive", true);
    let store = PatternStore::with_cache(db, &config.patterns, cache);
    store.record_feedback(pattern_id, positive)?;
    Ok(json!({"status": "ok", "pattern_id": pattern_id, "updated": true}))
}

pub fn stats(db: &MemoryDb) -> flowforge_core::Result<Value> {
    let short = db.count_patterns_short().unwrap_or(0);
    let long = db.count_patterns_long().unwrap_or(0);
    let (routing_hits, routing_total) = db.routing_accuracy_stats().unwrap_or((0, 0));
    let (pattern_successes, pattern_total) = db.pattern_hit_rate().unwrap_or((0, 0));
    let (with_conf, without_conf, with_count, without_count) =
        db.context_effectiveness_stats().unwrap_or((0.0, 0.0, 0, 0));

    Ok(json!({
        "status": "ok",
        "short_term_count": short,
        "long_term_count": long,
        "total": short + long,
        "context_effectiveness": {
            "routing_accuracy": {
                "hits": routing_hits,
                "total": routing_total,
                "rate": if routing_total > 0 { routing_hits as f64 / routing_total as f64 } else { 0.0 },
            },
            "pattern_hit_rate": {
                "successes": pattern_successes,
                "total": pattern_total,
                "rate": if pattern_total > 0 { pattern_successes as f64 / pattern_total as f64 } else { 0.0 },
            },
            "avg_confidence": {
                "with_context": with_conf,
                "without_context": without_conf,
                "with_count": with_count,
                "without_count": without_count,
            },
        },
    }))
}

pub fn clusters(db: &MemoryDb) -> flowforge_core::Result<Value> {
    let clusters = db.get_all_clusters().unwrap_or_default();
    let outlier_count = db.count_outlier_vectors().unwrap_or(0);
    let cluster_info: Vec<Value> = clusters
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "member_count": c.member_count,
                "p95_distance": c.p95_distance,
                "avg_confidence": c.avg_confidence,
            })
        })
        .collect();
    Ok(json!({
        "status": "ok",
        "cluster_count": clusters.len(),
        "outlier_count": outlier_count,
        "clusters": cluster_info,
    }))
}
