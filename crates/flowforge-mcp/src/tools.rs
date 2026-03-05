use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use flowforge_agents::{AgentRegistry, AgentRouter};
use flowforge_core::FlowForgeConfig;
use flowforge_memory::{MemoryDb, PatternStore};
use flowforge_tmux::TmuxStateManager;

use crate::params::ParamExt;
use crate::tool_builder::ToolBuilderExt;

pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct ToolRegistry {
    tools: HashMap<String, ToolDef>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut tools = HashMap::new();
        Self::register_all(&mut tools);
        Self { tools }
    }

    pub fn list(&self) -> Vec<&ToolDef> {
        let mut tools: Vec<_> = self.tools.values().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }

    pub fn get(&self, name: &str) -> Option<&ToolDef> {
        self.tools.get(name)
    }

    pub fn call(&self, name: &str, params: &Value) -> Value {
        match name {
            "memory_get" => Self::memory_get(params),
            "memory_set" => Self::memory_set(params),
            "memory_search" => Self::memory_search(params),
            "memory_delete" => Self::memory_delete(params),
            "memory_list" => Self::memory_list(params),
            "memory_import" => Self::memory_import(params),
            "learning_store" => Self::learning_store(params),
            "learning_search" => Self::learning_search(params),
            "learning_feedback" => Self::learning_feedback(params),
            "learning_stats" => Self::learning_stats(params),
            "learning_clusters" => Self::learning_clusters(params),
            "agents_list" => Self::agents_list(params),
            "agents_route" => Self::agents_route(params),
            "agents_info" => Self::agents_info(params),
            "session_status" => Self::session_status(params),
            "session_metrics" => Self::session_metrics(params),
            "session_history" => Self::session_history(params),
            "session_agents" => Self::session_agents(params),
            "team_status" => Self::team_status(params),
            "team_log" => Self::team_log(params),
            "work_create" => Self::work_create(params),
            "work_list" => Self::work_list(params),
            "work_update" => Self::work_update(params),
            "work_log" => Self::work_log(params),
            "conversation_history" => Self::conversation_history(params),
            "conversation_search" => Self::conversation_search(params),
            "conversation_ingest" => Self::conversation_ingest(params),
            "checkpoint_create" => Self::checkpoint_create(params),
            "checkpoint_list" => Self::checkpoint_list(params),
            "checkpoint_get" => Self::checkpoint_get(params),
            "session_fork" => Self::session_fork(params),
            "session_forks" => Self::session_forks(params),
            "session_lineage" => Self::session_lineage(params),
            "mailbox_send" => Self::mailbox_send(params),
            "mailbox_read" => Self::mailbox_read(params),
            "mailbox_history" => Self::mailbox_history(params),
            "mailbox_agents" => Self::mailbox_agents(params),
            "guidance_rules" => Self::guidance_rules(params),
            "guidance_trust" => Self::guidance_trust(params),
            "guidance_audit" => Self::guidance_audit(params),
            "work_claim" => Self::work_claim(params),
            "work_release" => Self::work_release(params),
            "work_steal" => Self::work_steal(params),
            "work_heartbeat" => Self::work_heartbeat(params),
            "plugin_list" => Self::plugin_list(params),
            "plugin_info" => Self::plugin_info(params),
            "trajectory_list" => Self::trajectory_list(params),
            "trajectory_get" => Self::trajectory_get(params),
            "trajectory_judge" => Self::trajectory_judge(params),
            "work_close" => Self::work_close(params),
            "work_sync" => Self::work_sync(params),
            "work_load" => Self::work_load(params),
            "guidance_verify" => Self::guidance_verify(params),
            "work_stealable" => Self::work_stealable(params),
            "work_status" => Self::work_status(params),
            _ => json!({ "error": format!("unknown tool: {}", name) }),
        }
    }

    // ── Registration ───────────────────────────────────────────────

    fn register_all(tools: &mut HashMap<String, ToolDef>) {
        // Memory tools
        tools
            .tool("memory_get", "Get a memory entry by key")
            .required_str("key", "The memory key to retrieve")
            .build();
        tools
            .tool("memory_set", "Store a memory entry with a key and value")
            .required_str("key", "The memory key")
            .required_str("value", "The value to store")
            .optional_str("category", "Optional category for the memory")
            .build();
        tools
            .tool("memory_search", "Search memory entries by query string")
            .required_str("query", "Search query")
            .optional_int_default("limit", "Max results to return", 10)
            .build();
        tools
            .tool("memory_delete", "Delete a memory entry by key")
            .required_str("key", "The memory key to delete")
            .build();
        tools
            .tool(
                "memory_list",
                "List all memory entries, optionally filtered by category",
            )
            .optional_str("category", "Filter by category")
            .optional_int_default("limit", "Max results", 50)
            .build();
        tools
            .tool("memory_import", "Import memory entries from a JSON array")
            .required_array(
                "entries",
                "Array of memory entries to import",
                json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" },
                        "value": { "type": "string" },
                        "category": { "type": "string" }
                    },
                    "required": ["key", "value"]
                }),
            )
            .build();

        // Learning tools
        tools
            .tool(
                "learning_store",
                "Store a learned pattern from an observation",
            )
            .required_str("content", "The pattern content")
            .required_str("category", "Pattern category (e.g., code_style, error_fix)")
            .optional_num_default("confidence", "Initial confidence 0.0-1.0", 0.5)
            .build();
        tools
            .tool("learning_search", "Search learned patterns by query")
            .required_str("query", "Search query")
            .optional_str("category", "Filter by category")
            .optional_int_default("limit", "Max results", 10)
            .build();
        tools
            .tool(
                "learning_feedback",
                "Provide feedback on a learned pattern (positive or negative)",
            )
            .required_str("pattern_id", "The pattern ID")
            .required_bool("positive", "Whether the feedback is positive")
            .build();
        tools
            .tool("learning_stats", "Get statistics about learned patterns")
            .build();
        tools
            .tool(
                "learning_clusters",
                "Get topic cluster information for pattern vectors",
            )
            .build();

        // Agent tools
        tools
            .tool(
                "agents_list",
                "List all available agents with their capabilities",
            )
            .optional_str("source", "Filter by source: builtin, global, project")
            .build();
        tools
            .tool(
                "agents_route",
                "Route a task description to the best matching agent",
            )
            .required_str("task", "Task description to route")
            .optional_int_default("top_k", "Number of top candidates", 3)
            .build();
        tools
            .tool("agents_info", "Get detailed info about a specific agent")
            .required_str("name", "Agent name")
            .build();

        // Session tools
        tools
            .tool(
                "session_status",
                "Get current session status including active tasks and edits",
            )
            .build();
        tools
            .tool(
                "session_metrics",
                "Get session metrics: edits, commands, routing decisions",
            )
            .optional_str("session_id", "Session ID (defaults to current)")
            .build();
        tools
            .tool("session_history", "Get session history with summaries")
            .optional_int_default("limit", "Max sessions to return", 10)
            .build();
        tools
            .tool(
                "session_agents",
                "List agent sessions for a given session or the current session",
            )
            .optional_str("session_id", "Parent session ID (defaults to current)")
            .build();

        // Team tools
        tools
            .tool(
                "team_status",
                "Get current team status including all member states",
            )
            .build();
        tools
            .tool("team_log", "Get recent team activity log")
            .optional_int_default("limit", "Max log entries", 20)
            .build();

        // Work tracking tools
        tools
            .tool(
                "work_create",
                "Create a new work item (task, epic, bug, story)",
            )
            .required_str("title", "Title of the work item")
            .optional_str_default(
                "type",
                "Item type: task, epic, bug, story, sub-task",
                "task",
            )
            .optional_str("description", "Optional description")
            .optional_str("parent_id", "Parent work item ID for hierarchy")
            .optional_int_default("priority", "Priority 0-3 (0=critical)", 2)
            .build();
        tools
            .tool("work_list", "List work items with optional filters")
            .optional_str(
                "status",
                "Filter by status: pending, in_progress, blocked, completed",
            )
            .optional_str("type", "Filter by item type")
            .optional_int_default("limit", "Max results", 20)
            .build();
        tools
            .tool("work_update", "Update a work item's status")
            .required_str("id", "Work item ID")
            .required_str(
                "status",
                "New status: pending, in_progress, blocked, completed",
            )
            .build();
        tools
            .tool("work_log", "Query the work tracking audit trail")
            .optional_str("work_item_id", "Filter by work item ID (optional)")
            .optional_int_default("limit", "Max events", 20)
            .build();

        // Conversation tools
        tools
            .tool(
                "conversation_history",
                "Get conversation messages for a session (paginated)",
            )
            .required_str("session_id", "Session ID")
            .optional_int_default("limit", "Max messages", 20)
            .optional_int_default("offset", "Offset for pagination", 0)
            .build();
        tools
            .tool(
                "conversation_search",
                "Search conversation messages by content (LIKE search)",
            )
            .required_str("session_id", "Session ID")
            .required_str("query", "Search query")
            .optional_int_default("limit", "Max results", 10)
            .build();
        tools
            .tool(
                "conversation_ingest",
                "Trigger transcript ingestion for a session",
            )
            .required_str("session_id", "Session ID")
            .required_str("transcript_path", "Path to JSONL transcript")
            .build();

        // Checkpoint tools
        tools
            .tool(
                "checkpoint_create",
                "Create a named checkpoint at the current conversation position",
            )
            .required_str("session_id", "Session ID")
            .required_str("name", "Checkpoint name")
            .optional_str("description", "Optional description")
            .build();
        tools
            .tool("checkpoint_list", "List checkpoints for a session")
            .required_str("session_id", "Session ID")
            .build();
        tools
            .tool(
                "checkpoint_get",
                "Get a checkpoint by ID or by name+session",
            )
            .optional_str("id", "Checkpoint ID")
            .optional_str("session_id", "Session ID (for name lookup)")
            .optional_str("name", "Checkpoint name (requires session_id)")
            .build();

        // Session fork tools
        tools
            .tool(
                "session_fork",
                "Fork a session's conversation at a checkpoint or message index",
            )
            .required_str("session_id", "Source session ID")
            .optional_str("checkpoint_name", "Fork at this checkpoint")
            .optional_int("at_index", "Fork at this message index")
            .optional_str("reason", "Reason for the fork")
            .build();
        tools
            .tool("session_forks", "List forks for a session")
            .required_str("session_id", "Session ID")
            .build();
        tools
            .tool(
                "session_lineage",
                "Trace the fork lineage of a session back to root",
            )
            .required_str("session_id", "Session ID")
            .build();

        // Mailbox tools
        tools
            .tool("mailbox_send", "Send a message to co-agents on a work item")
            .required_str("work_item_id", "Work item ID (coordination hub)")
            .required_str("from_session_id", "Sender session ID")
            .required_str("from_agent_name", "Sender agent name")
            .optional_str("to_session_id", "Target session ID (omit for broadcast)")
            .optional_str("to_agent_name", "Target agent name (omit for broadcast)")
            .required_str("content", "Message content")
            .optional_str_default(
                "message_type",
                "Message type: text, status_update, request, result",
                "text",
            )
            .optional_int_default("priority", "Priority 0-3 (0=highest)", 2)
            .build();
        tools
            .tool("mailbox_read", "Read unread mailbox messages for a session")
            .required_str("session_id", "Session ID")
            .build();
        tools
            .tool(
                "mailbox_history",
                "Get mailbox message history for a work item",
            )
            .required_str("work_item_id", "Work item ID")
            .optional_int_default("limit", "Max messages", 20)
            .build();
        tools
            .tool("mailbox_agents", "List agents assigned to a work item")
            .required_str("work_item_id", "Work item ID")
            .build();

        // Guidance tools
        tools
            .tool(
                "guidance_rules",
                "List guidance rules and gate configuration",
            )
            .build();
        tools
            .tool("guidance_trust", "Get trust score for a session")
            .optional_str("session_id", "Session ID (optional, defaults to current)")
            .build();
        tools
            .tool("guidance_audit", "Get gate decision audit trail")
            .optional_str("session_id", "Session ID")
            .optional_int("limit", "Max results (default 20)")
            .build();

        // Work-stealing tools
        tools
            .tool("work_claim", "Claim a work item for the current session")
            .required_str("id", "Work item ID")
            .build();
        tools
            .tool("work_release", "Release a claimed work item")
            .required_str("id", "Work item ID")
            .build();
        tools
            .tool("work_steal", "Steal a stealable work item")
            .optional_str(
                "id",
                "Work item ID (optional, steals highest priority if omitted)",
            )
            .build();
        tools
            .tool("work_heartbeat", "Update heartbeat for claimed work items")
            .optional_int("progress", "Progress percentage (0-100)")
            .optional_str("id", "Work item ID for progress update")
            .build();

        // Plugin tools
        tools.tool("plugin_list", "List installed plugins").build();
        tools
            .tool("plugin_info", "Get detailed plugin information")
            .required_str("name", "Plugin name")
            .build();

        // Trajectory tools
        tools
            .tool("trajectory_list", "List recorded trajectories")
            .optional_str("session_id", "Session ID")
            .optional_str(
                "status",
                "Filter by status: recording, completed, failed, judged",
            )
            .optional_int("limit", "Max results (default 20)")
            .build();
        tools
            .tool("trajectory_get", "Get trajectory details with steps")
            .required_str("id", "Trajectory ID")
            .build();
        tools
            .tool("trajectory_judge", "Judge a completed trajectory")
            .required_str("id", "Trajectory ID")
            .build();

        // Work close/sync/load tools
        tools
            .tool("work_close", "Close a work item (set status to completed)")
            .required_str("id", "Work item ID to close")
            .build();
        tools
            .tool(
                "work_sync",
                "Sync work items with external backend (kanbus/beads/claude_tasks)",
            )
            .build();
        tools
            .tool("work_load", "Show work distribution across agents")
            .build();

        // Guidance verify
        tools
            .tool(
                "guidance_verify",
                "Verify SHA-256 audit hash chain integrity",
            )
            .optional_str("session_id", "Session ID (optional, defaults to current)")
            .build();

        // Work stealable / status
        tools
            .tool(
                "work_stealable",
                "List work items that are available for stealing",
            )
            .optional_int("limit", "Max items to return (default 10)")
            .build();
        tools
            .tool("work_status", "Get work item counts by status")
            .build();
    }

    // ── Helpers ────────────────────────────────────────────────────

    fn with_db<F>(f: F) -> Value
    where
        F: FnOnce(&MemoryDb, &FlowForgeConfig) -> flowforge_core::Result<Value>,
    {
        match FlowForgeConfig::load(&FlowForgeConfig::config_path()) {
            Ok(config) => match MemoryDb::open(&config.db_path()) {
                Ok(db) => match f(&db, &config) {
                    Ok(v) => v,
                    Err(e) => json!({"status": "error", "message": format!("{e}")}),
                },
                Err(e) => {
                    json!({"status": "error", "message": format!("Failed to open database: {e}")})
                }
            },
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn with_config<F>(f: F) -> Value
    where
        F: FnOnce(&FlowForgeConfig) -> flowforge_core::Result<Value>,
    {
        match FlowForgeConfig::load(&FlowForgeConfig::config_path()) {
            Ok(config) => match f(&config) {
                Ok(v) => v,
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn current_session_id(db: &MemoryDb) -> String {
        db.get_current_session()
            .ok()
            .flatten()
            .map(|s| s.id)
            .unwrap_or_else(|| "unknown".to_string())
    }

    // ── Memory tools ──────────────────────────────────────────────

    fn memory_get(p: &Value) -> Value {
        let key = p.str_or("key", "");
        let namespace = p.str_or("namespace", "default");
        Self::with_db(|db, _| {
            let value = db.kv_get(key, namespace)?;
            Ok(json!({"status": "ok", "key": key, "value": value}))
        })
    }

    fn memory_set(p: &Value) -> Value {
        let key = p.str_or("key", "");
        let value = p.str_or("value", "");
        let namespace = p
            .opt_str("namespace")
            .or_else(|| p.opt_str("category"))
            .unwrap_or("default");
        Self::with_db(|db, _| {
            db.kv_set(key, value, namespace)?;
            Ok(json!({"status": "ok", "key": key, "stored": true}))
        })
    }

    fn memory_search(p: &Value) -> Value {
        let query = p.str_or("query", "");
        let limit = p.u64_or("limit", 10) as usize;
        Self::with_db(|db, _| {
            let results = db.kv_search(query, limit)?;
            let entries: Vec<Value> = results
                .iter()
                .map(|(k, v, ns)| json!({"key": k, "value": v, "namespace": ns}))
                .collect();
            Ok(json!({"status": "ok", "query": query, "results": entries}))
        })
    }

    fn memory_delete(p: &Value) -> Value {
        let key = p.str_or("key", "");
        let namespace = p.str_or("namespace", "default");
        Self::with_db(|db, _| {
            db.kv_delete(key, namespace)?;
            Ok(json!({"status": "ok", "key": key, "deleted": true}))
        })
    }

    fn memory_list(p: &Value) -> Value {
        let namespace = p.str_or("category", "default");
        let limit = p.u64_or("limit", 50) as usize;
        Self::with_db(|db, _| {
            let entries = db.kv_list(namespace)?;
            let entries: Vec<Value> = entries
                .iter()
                .take(limit)
                .map(|(k, v)| json!({"key": k, "value": v}))
                .collect();
            Ok(json!({"status": "ok", "entries": entries}))
        })
    }

    fn memory_import(p: &Value) -> Value {
        let entries = match p.get("entries").and_then(|v| v.as_array()) {
            Some(arr) => arr.clone(),
            None => return json!({"status": "error", "message": "missing entries array"}),
        };
        let total = entries.len();
        Self::with_db(|db, _| {
            let mut imported = 0usize;
            for entry in &entries {
                let key = entry.str_or("key", "");
                let value = entry.str_or("value", "");
                let namespace = entry
                    .opt_str("namespace")
                    .or_else(|| entry.opt_str("category"))
                    .unwrap_or("default");
                if db.kv_set(key, value, namespace).is_ok() {
                    imported += 1;
                }
            }
            Ok(json!({"status": "ok", "imported": imported, "total": total}))
        })
    }

    // ── Learning tools ────────────────────────────────────────────

    fn learning_store(p: &Value) -> Value {
        let content = p.str_or("content", "");
        let category = p.str_or("category", "");
        Self::with_db(|db, config| {
            let store = PatternStore::new(db, &config.patterns);
            let id = store.store_short_term(content, category)?;
            Ok(json!({"status": "ok", "pattern_id": id}))
        })
    }

    fn learning_search(p: &Value) -> Value {
        let query = p.str_or("query", "");
        let limit = p.u64_or("limit", 10) as usize;
        Self::with_db(|db, config| {
            let store = PatternStore::new(db, &config.patterns);
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
        })
    }

    fn learning_feedback(p: &Value) -> Value {
        let pattern_id = p.str_or("pattern_id", "");
        let positive = p.bool_or("positive", true);
        Self::with_db(|db, config| {
            let store = PatternStore::new(db, &config.patterns);
            store.record_feedback(pattern_id, positive)?;
            Ok(json!({"status": "ok", "pattern_id": pattern_id, "updated": true}))
        })
    }

    fn learning_stats(_p: &Value) -> Value {
        Self::with_db(|db, _| {
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
        })
    }

    fn learning_clusters(_p: &Value) -> Value {
        Self::with_db(|db, _| {
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
        })
    }

    // ── Agent tools ───────────────────────────────────────────────

    fn agents_list(p: &Value) -> Value {
        let source_filter = p.opt_str("source");
        Self::with_config(|config| {
            let registry = AgentRegistry::load(&config.agents)?;
            let agents: Vec<Value> = registry
                .list()
                .iter()
                .filter(|a| {
                    source_filter
                        .map(|s| {
                            format!("{:?}", a.source)
                                .to_lowercase()
                                .contains(&s.to_lowercase())
                        })
                        .unwrap_or(true)
                })
                .map(|a| {
                    json!({
                        "name": a.name,
                        "description": a.description,
                        "capabilities": a.capabilities,
                        "source": format!("{:?}", a.source),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "agents": agents}))
        })
    }

    fn agents_route(p: &Value) -> Value {
        let task = p.str_or("task", "");
        let top_k = p.u64_or("top_k", 3) as usize;
        Self::with_db(|db, config| {
            let registry = AgentRegistry::load(&config.agents)?;
            let router = AgentRouter::new(&config.routing);
            let weights_vec = db.get_all_routing_weights()?;
            let mut learned_weights: HashMap<(String, String), f64> = HashMap::new();
            for w in &weights_vec {
                learned_weights.insert((w.task_pattern.clone(), w.agent_name.clone()), w.weight);
            }
            let agent_refs: Vec<&_> = registry.list();
            let results = router.route(task, &agent_refs, &learned_weights, None);
            let candidates: Vec<Value> = results
                .iter()
                .take(top_k)
                .map(|r| {
                    json!({
                        "agent_name": r.agent_name,
                        "confidence": r.confidence,
                        "breakdown": {
                            "pattern_score": r.breakdown.pattern_score,
                            "capability_score": r.breakdown.capability_score,
                            "learned_score": r.breakdown.learned_score,
                            "context_score": r.breakdown.context_score,
                            "priority_score": r.breakdown.priority_score,
                        },
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "candidates": candidates}))
        })
    }

    fn agents_info(p: &Value) -> Value {
        let name = p.str_or("name", "");
        Self::with_config(|config| {
            let registry = AgentRegistry::load(&config.agents)?;
            match registry.get(name) {
                Some(agent) => Ok(json!({
                    "status": "ok",
                    "agent": {
                        "name": agent.name,
                        "description": agent.description,
                        "capabilities": agent.capabilities,
                        "patterns": agent.patterns,
                        "priority": format!("{:?}", agent.priority),
                        "source": format!("{:?}", agent.source),
                        "body": agent.body,
                    },
                })),
                None => Ok(json!({"status": "error", "message": "Agent not found"})),
            }
        })
    }

    // ── Session tools ─────────────────────────────────────────────

    fn session_status(_p: &Value) -> Value {
        Self::with_db(|db, _| match db.get_current_session()? {
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
        })
    }

    fn session_metrics(p: &Value) -> Value {
        let session_id = p.opt_str("session_id");
        Self::with_db(|db, _| {
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
                None => {
                    Ok(json!({"status": "ok", "session_id": session_id, "edits": 0, "commands": 0}))
                }
            }
        })
    }

    fn session_history(p: &Value) -> Value {
        let limit = p.u64_or("limit", 10) as usize;
        Self::with_db(|db, _| {
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
        })
    }

    fn session_agents(p: &Value) -> Value {
        let session_id = p.opt_str("session_id");
        Self::with_db(|db, _| {
            let parent_id = if let Some(id) = session_id {
                id.to_string()
            } else {
                match db.get_current_session()? {
                    Some(s) => s.id,
                    None => return Ok(json!({"status": "ok", "agents": [], "count": 0})),
                }
            };
            let agents = db.get_agent_sessions(&parent_id)?;
            let entries: Vec<Value> = agents
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
        })
    }

    // ── Team tools ────────────────────────────────────────────────

    fn team_status(_p: &Value) -> Value {
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

    fn team_log(p: &Value) -> Value {
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

    // ── Work tracking tools ───────────────────────────────────────

    fn work_create(p: &Value) -> Value {
        let title = p.str_or("title", "");
        let item_type = p.str_or("type", "task");
        let description = p.opt_str("description");
        let parent_id = p.opt_str("parent_id");
        let priority = p.i64_or("priority", 2) as i32;
        Self::with_db(|db, config| {
            let now = chrono::Utc::now();
            let backend =
                flowforge_core::work_tracking::detect_backend(&config.work_tracking).to_string();
            let item = flowforge_core::WorkItem {
                id: uuid::Uuid::new_v4().to_string(),
                external_id: None,
                backend,
                item_type: item_type.to_string(),
                title: title.to_string(),
                description: description.map(|s| s.to_string()),
                status: "pending".to_string(),
                assignee: None,
                parent_id: parent_id.map(|s| s.to_string()),
                priority,
                labels: vec![],
                created_at: now,
                updated_at: now,
                completed_at: None,
                session_id: None,
                metadata: None,
                claimed_by: None,
                claimed_at: None,
                last_heartbeat: None,
                progress: 0,
                stealable: false,
            };
            flowforge_core::work_tracking::create_item(db, &config.work_tracking, &item)?;
            Ok(json!({"status": "ok", "id": item.id, "title": title}))
        })
    }

    fn work_list(p: &Value) -> Value {
        let status = p.opt_str("status");
        let item_type = p.opt_str("type");
        let limit = p.u64_or("limit", 20) as usize;
        Self::with_db(|db, _| {
            let filter = flowforge_core::WorkFilter {
                status: status.map(|s| s.to_string()),
                item_type: item_type.map(|s| s.to_string()),
                limit: Some(limit),
                ..Default::default()
            };
            let items = db.list_work_items(&filter)?;
            let entries: Vec<Value> = items
                .iter()
                .map(|i| {
                    json!({
                        "id": i.id,
                        "title": i.title,
                        "type": i.item_type,
                        "status": i.status,
                        "assignee": i.assignee,
                        "priority": i.priority,
                        "backend": i.backend,
                        "created_at": i.created_at.to_rfc3339(),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "items": entries, "count": entries.len()}))
        })
    }

    fn work_update(p: &Value) -> Value {
        let id = p.str_or("id", "");
        let new_status = p.str_or("status", "");
        Self::with_db(|db, config| {
            flowforge_core::work_tracking::update_status(
                db,
                &config.work_tracking,
                id,
                new_status,
                "mcp",
            )?;
            Ok(json!({"status": "ok", "id": id, "new_status": new_status}))
        })
    }

    fn work_log(p: &Value) -> Value {
        let work_item_id = p.opt_str("work_item_id");
        let limit = p.u64_or("limit", 20) as usize;
        Self::with_db(|db, _| {
            let events = if let Some(id) = work_item_id {
                db.get_work_events(id, limit)?
            } else {
                db.get_recent_work_events(limit)?
            };
            let entries: Vec<Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "work_item_id": e.work_item_id,
                        "event_type": e.event_type,
                        "old_value": e.old_value,
                        "new_value": e.new_value,
                        "actor": e.actor,
                        "timestamp": e.timestamp.to_rfc3339(),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "events": entries}))
        })
    }

    // ── Conversation tools ────────────────────────────────────────

    fn conversation_history(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        let limit = p.u64_or("limit", 20) as usize;
        let offset = p.u64_or("offset", 0) as usize;
        Self::with_db(|db, _| {
            let total = db.get_conversation_message_count(session_id).unwrap_or(0);
            let msgs = db.get_conversation_messages(session_id, limit, offset)?;
            let entries: Vec<Value> = msgs
                .iter()
                .map(|m| {
                    json!({
                        "message_index": m.message_index,
                        "role": m.role,
                        "message_type": m.message_type,
                        "content": m.content,
                        "model": m.model,
                        "timestamp": m.timestamp.to_rfc3339(),
                        "source": m.source,
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "messages": entries, "total": total}))
        })
    }

    fn conversation_search(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        let query = p.str_or("query", "");
        let limit = p.u64_or("limit", 10) as usize;
        Self::with_db(|db, _| {
            let msgs = db.search_conversation_messages(session_id, query, limit)?;
            let entries: Vec<Value> = msgs
                .iter()
                .map(|m| {
                    json!({
                        "message_index": m.message_index,
                        "role": m.role,
                        "content": m.content,
                        "timestamp": m.timestamp.to_rfc3339(),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "results": entries, "count": entries.len()}))
        })
    }

    fn conversation_ingest(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        let path = p.str_or("transcript_path", "");
        Self::with_db(|db, _| {
            let count = db.ingest_transcript(session_id, path)?;
            Ok(json!({"status": "ok", "ingested": count, "session_id": session_id}))
        })
    }

    // ── Checkpoint tools ──────────────────────────────────────────

    fn checkpoint_create(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        let name = p.str_or("name", "");
        let description = p.opt_str("description");
        Self::with_db(|db, _| {
            let message_index = db.get_latest_message_index(session_id).unwrap_or(0);
            let cp = flowforge_core::Checkpoint {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.to_string(),
                name: name.to_string(),
                message_index,
                description: description.map(|s| s.to_string()),
                git_ref: None,
                created_at: chrono::Utc::now(),
                metadata: None,
            };
            db.create_checkpoint(&cp)?;
            Ok(json!({"status": "ok", "id": cp.id, "name": name, "message_index": message_index}))
        })
    }

    fn checkpoint_list(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        Self::with_db(|db, _| {
            let cps = db.list_checkpoints(session_id)?;
            let entries: Vec<Value> = cps
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "name": c.name,
                        "message_index": c.message_index,
                        "description": c.description,
                        "git_ref": c.git_ref,
                        "created_at": c.created_at.to_rfc3339(),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "checkpoints": entries}))
        })
    }

    fn checkpoint_get(p: &Value) -> Value {
        let id = p.opt_str("id");
        let session_id = p.opt_str("session_id");
        let name = p.opt_str("name");
        Self::with_db(|db, _| {
            let cp = if let Some(id) = id {
                db.get_checkpoint(id)?
            } else if let (Some(sid), Some(n)) = (session_id, name) {
                db.get_checkpoint_by_name(sid, n)?
            } else {
                return Ok(
                    json!({"status": "error", "message": "Provide either id or session_id+name"}),
                );
            };
            match cp {
                Some(c) => Ok(json!({
                    "status": "ok",
                    "checkpoint": {
                        "id": c.id,
                        "session_id": c.session_id,
                        "name": c.name,
                        "message_index": c.message_index,
                        "description": c.description,
                        "git_ref": c.git_ref,
                        "created_at": c.created_at.to_rfc3339(),
                    }
                })),
                None => Ok(json!({"status": "error", "message": "Checkpoint not found"})),
            }
        })
    }

    // ── Session fork tools ────────────────────────────────────────

    fn session_fork(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        let checkpoint_name = p.opt_str("checkpoint_name");
        let at_index = p.opt_u32("at_index");
        let reason = p.opt_str("reason");
        Self::with_db(|db, _| {
            let (fork_index, checkpoint_id) = if let Some(cp_name) = checkpoint_name {
                match db.get_checkpoint_by_name(session_id, cp_name)? {
                    Some(cp) => (cp.message_index, Some(cp.id)),
                    None => {
                        return Ok(
                            json!({"status": "error", "message": format!("Checkpoint '{}' not found", cp_name)}),
                        )
                    }
                }
            } else if let Some(idx) = at_index {
                (idx, None)
            } else {
                let latest = db.get_latest_message_index(session_id).unwrap_or(0);
                (latest.saturating_sub(1), None)
            };

            let new_session_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let new_session = flowforge_core::SessionInfo {
                id: new_session_id.clone(),
                started_at: now,
                ended_at: None,
                cwd: ".".to_string(),
                edits: 0,
                commands: 0,
                summary: Some(format!("Forked from {}", session_id)),
                transcript_path: None,
            };
            db.create_session(&new_session)?;

            let copied = db.fork_conversation(session_id, &new_session_id, fork_index)?;

            let fork = flowforge_core::SessionFork {
                id: uuid::Uuid::new_v4().to_string(),
                source_session_id: session_id.to_string(),
                target_session_id: new_session_id.clone(),
                fork_message_index: fork_index,
                checkpoint_id,
                reason: reason.map(|s| s.to_string()),
                created_at: now,
            };
            db.create_session_fork(&fork)?;

            Ok(json!({
                "status": "ok",
                "fork_id": fork.id,
                "new_session_id": new_session_id,
                "fork_message_index": fork_index,
                "messages_copied": copied,
            }))
        })
    }

    fn session_forks(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        Self::with_db(|db, _| {
            let forks = db.get_session_forks(session_id)?;
            let entries: Vec<Value> = forks
                .iter()
                .map(|f| {
                    json!({
                        "id": f.id,
                        "source_session_id": f.source_session_id,
                        "target_session_id": f.target_session_id,
                        "fork_message_index": f.fork_message_index,
                        "checkpoint_id": f.checkpoint_id,
                        "reason": f.reason,
                        "created_at": f.created_at.to_rfc3339(),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "forks": entries}))
        })
    }

    fn session_lineage(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        Self::with_db(|db, _| {
            let lineage = db.get_session_lineage(session_id)?;
            let entries: Vec<Value> = lineage
                .iter()
                .map(|f| {
                    json!({
                        "source_session_id": f.source_session_id,
                        "target_session_id": f.target_session_id,
                        "fork_message_index": f.fork_message_index,
                        "created_at": f.created_at.to_rfc3339(),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "lineage": entries, "depth": entries.len()}))
        })
    }

    // ── Mailbox tools ─────────────────────────────────────────────

    fn mailbox_send(p: &Value) -> Value {
        let work_item_id = p.str_or("work_item_id", "");
        let from_session_id = p.str_or("from_session_id", "");
        let from_agent_name = p.str_or("from_agent_name", "");
        let to_session_id = p.opt_str("to_session_id");
        let to_agent_name = p.opt_str("to_agent_name");
        let content = p.str_or("content", "");
        let message_type = p.str_or("message_type", "text");
        let priority = p.i64_or("priority", 2) as i32;
        Self::with_db(|db, _| {
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
        })
    }

    fn mailbox_read(p: &Value) -> Value {
        let session_id = p.str_or("session_id", "");
        Self::with_db(|db, _| {
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
        })
    }

    fn mailbox_history(p: &Value) -> Value {
        let work_item_id = p.str_or("work_item_id", "");
        let limit = p.u64_or("limit", 20) as usize;
        Self::with_db(|db, _| {
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
        })
    }

    fn mailbox_agents(p: &Value) -> Value {
        let work_item_id = p.str_or("work_item_id", "");
        Self::with_db(|db, _| {
            let agents = db.get_agents_on_work_item(work_item_id)?;
            let entries: Vec<Value> = agents
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
        })
    }

    // ── Guidance tools ────────────────────────────────────────────

    fn guidance_rules(_p: &Value) -> Value {
        Self::with_config(|config| {
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
        })
    }

    fn guidance_trust(p: &Value) -> Value {
        let session_id = p.opt_str("session_id");
        Self::with_db(|db, _| {
            let sid = match session_id {
                Some(s) => s.to_string(),
                None => Self::current_session_id(db),
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
        })
    }

    fn guidance_audit(p: &Value) -> Value {
        let session_id = p.opt_str("session_id");
        let limit = p.u64_or("limit", 20) as usize;
        Self::with_db(|db, _| {
            let sid = match session_id {
                Some(s) => s.to_string(),
                None => Self::current_session_id(db),
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
        })
    }

    // ── Work-stealing tools ───────────────────────────────────────

    fn work_claim(p: &Value) -> Value {
        let id = p.str_or("id", "");
        Self::with_db(|db, _| {
            let session_id = Self::current_session_id(db);
            let claimed = db.claim_work_item(id, &session_id)?;
            Ok(json!({"status": "ok", "claimed": claimed, "id": id}))
        })
    }

    fn work_release(p: &Value) -> Value {
        let id = p.str_or("id", "");
        Self::with_db(|db, _| {
            db.release_work_item(id)?;
            Ok(json!({"status": "ok", "id": id}))
        })
    }

    fn work_steal(p: &Value) -> Value {
        let id = p.opt_str("id");
        Self::with_db(|db, _| {
            let session_id = Self::current_session_id(db);
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
        })
    }

    fn work_heartbeat(p: &Value) -> Value {
        let progress = p.opt_i64("progress").map(|v| v as i32);
        let id = p.opt_str("id");
        Self::with_db(|db, _| {
            let session_id = Self::current_session_id(db);
            let updated = db.update_heartbeat(&session_id)?;
            if let (Some(id), Some(progress)) = (id, progress) {
                db.update_progress(id, progress)?;
            }
            Ok(json!({"status": "ok", "items_updated": updated}))
        })
    }

    // ── Plugin tools ──────────────────────────────────────────────

    fn plugin_list(_p: &Value) -> Value {
        Self::with_config(|config| {
            let plugins = flowforge_core::plugin::load_all_plugins(&config.plugins)?;
            let entries: Vec<Value> = plugins
                .iter()
                .map(|p| {
                    let disabled = config.plugins.disabled.contains(&p.manifest.plugin.name);
                    json!({
                        "name": p.manifest.plugin.name,
                        "version": p.manifest.plugin.version,
                        "description": p.manifest.plugin.description,
                        "enabled": !disabled,
                        "tools": p.manifest.tools.len(),
                        "hooks": p.manifest.hooks.len(),
                        "agents": p.manifest.agents.len(),
                    })
                })
                .collect();
            Ok(json!({"status": "ok", "count": entries.len(), "plugins": entries}))
        })
    }

    fn plugin_info(p: &Value) -> Value {
        let name = p.str_or("name", "");
        Self::with_config(|config| {
            let plugins = flowforge_core::plugin::load_all_plugins(&config.plugins)?;
            match plugins.iter().find(|p| p.manifest.plugin.name == name) {
                Some(p) => {
                    let disabled = config.plugins.disabled.contains(&p.manifest.plugin.name);
                    let tools: Vec<Value> = p
                        .manifest
                        .tools
                        .iter()
                        .map(|t| {
                            json!({
                                "name": t.name,
                                "description": t.description,
                                "timeout": t.timeout
                            })
                        })
                        .collect();
                    let hooks: Vec<Value> = p
                        .manifest
                        .hooks
                        .iter()
                        .map(|h| {
                            json!({
                                "event": h.event,
                                "priority": h.priority
                            })
                        })
                        .collect();
                    Ok(json!({
                        "status": "ok",
                        "name": name,
                        "version": p.manifest.plugin.version,
                        "description": p.manifest.plugin.description,
                        "enabled": !disabled,
                        "tools": tools,
                        "hooks": hooks
                    }))
                }
                None => {
                    Ok(json!({"status": "error", "message": format!("plugin '{name}' not found")}))
                }
            }
        })
    }

    // ── Trajectory tools ──────────────────────────────────────────

    fn trajectory_list(p: &Value) -> Value {
        let session_id = p.opt_str("session_id");
        let status = p.opt_str("status");
        let limit = p.u64_or("limit", 20) as usize;
        Self::with_db(|db, _| {
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
        })
    }

    fn trajectory_get(p: &Value) -> Value {
        let id = p.str_or("id", "");
        Self::with_db(|db, _| {
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
        })
    }

    fn trajectory_judge(p: &Value) -> Value {
        let id = p.str_or("id", "");
        Self::with_db(|db, config| {
            let judge = flowforge_memory::trajectory::TrajectoryJudge::new(db, &config.patterns);
            let result = judge.judge(id)?;
            Ok(json!({
                "status": "ok",
                "verdict": format!("{}", result.verdict),
                "confidence": result.confidence,
                "reason": result.reason
            }))
        })
    }

    // ── Work close/sync/load tools ────────────────────────────────

    fn work_close(p: &Value) -> Value {
        let id = p.str_or("id", "");
        Self::with_db(|db, config| {
            flowforge_core::work_tracking::close_item(db, &config.work_tracking, id, "mcp")?;
            Ok(json!({"status": "ok", "id": id}))
        })
    }

    fn work_sync(_p: &Value) -> Value {
        Self::with_db(|db, config| {
            let pulled =
                flowforge_core::work_tracking::sync_from_backend(db, &config.work_tracking)?;
            let pushed = flowforge_core::work_tracking::push_to_backend(db, &config.work_tracking)?;
            let backend =
                flowforge_core::work_tracking::detect_backend(&config.work_tracking).to_string();
            Ok(json!({
                "status": "ok",
                "pulled": pulled,
                "pushed": pushed,
                "backend": backend
            }))
        })
    }

    fn work_load(_p: &Value) -> Value {
        Self::with_db(|db, _| {
            let filter = flowforge_core::WorkFilter {
                status: Some("in_progress".to_string()),
                limit: Some(1000),
                ..Default::default()
            };
            let items = db.list_work_items(&filter)?;
            let mut by_agent: HashMap<String, Vec<Value>> = HashMap::new();
            for item in &items {
                let agent = item
                    .assignee
                    .clone()
                    .or_else(|| item.claimed_by.clone())
                    .unwrap_or_else(|| "unassigned".to_string());
                by_agent.entry(agent).or_default().push(json!({
                    "id": item.id,
                    "title": item.title,
                    "type": item.item_type,
                    "priority": item.priority,
                    "progress": item.progress,
                }));
            }
            let agents: Vec<Value> = by_agent
                .into_iter()
                .map(|(name, items)| json!({"name": name, "items": items}))
                .collect();
            let total = items.len();
            Ok(json!({"status": "ok", "agents": agents, "total": total}))
        })
    }

    // ── Guidance verify ───────────────────────────────────────────

    fn guidance_verify(p: &Value) -> Value {
        let session_id = p.opt_str("session_id");
        Self::with_db(|db, _| {
            let sid = match session_id {
                Some(s) => s.to_string(),
                None => Self::current_session_id(db),
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
                let expected_input =
                    format!("{}{}{}{}", d.session_id, d.tool_name, d.reason, prev_hash);
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
        })
    }

    fn work_stealable(p: &Value) -> Value {
        let limit = p.u64_or("limit", 10) as usize;
        Self::with_db(|db, _| {
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
        })
    }

    fn work_status(_p: &Value) -> Value {
        Self::with_db(|db, _| {
            let pending = db.count_work_items_by_status("pending").unwrap_or(0);
            let in_progress = db.count_work_items_by_status("in_progress").unwrap_or(0);
            let completed = db.count_work_items_by_status("completed").unwrap_or(0);
            let blocked = db.count_work_items_by_status("blocked").unwrap_or(0);
            let total = pending + in_progress + completed + blocked;
            Ok(json!({
                "status": "ok",
                "pending": pending,
                "in_progress": in_progress,
                "completed": completed,
                "blocked": blocked,
                "total": total
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_55_tools() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.list().len(), 55);
    }

    #[test]
    fn test_tool_lookup() {
        let registry = ToolRegistry::new();
        assert!(registry.get("memory_get").is_some());
        assert!(registry.get("team_log").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_tool_call() {
        let registry = ToolRegistry::new();
        let result = registry.call("memory_get", &json!({ "key": "test" }));
        assert!(result.get("status").is_some());
    }

    #[test]
    fn test_unknown_tool_call() {
        let registry = ToolRegistry::new();
        let result = registry.call("bogus", &json!({}));
        assert!(result["error"].as_str().unwrap().contains("unknown tool"));
    }

    #[test]
    fn test_all_tools_have_schemas() {
        let registry = ToolRegistry::new();
        for tool in registry.list() {
            assert_eq!(
                tool.input_schema["type"], "object",
                "tool {} missing schema",
                tool.name
            );
        }
    }

    #[test]
    fn test_new_tools_registered() {
        let registry = ToolRegistry::new();
        assert!(registry.get("work_close").is_some());
        assert!(registry.get("work_sync").is_some());
        assert!(registry.get("work_load").is_some());
        assert!(registry.get("guidance_verify").is_some());
        assert!(registry.get("work_stealable").is_some());
        assert!(registry.get("work_status").is_some());
    }

    #[test]
    fn test_work_close_requires_id() {
        let registry = ToolRegistry::new();
        let schema = &registry.get("work_close").unwrap().input_schema;
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("id")));
    }

    #[test]
    fn test_work_sync_no_required_params() {
        let registry = ToolRegistry::new();
        let schema = &registry.get("work_sync").unwrap().input_schema;
        assert!(schema.get("required").is_none());
    }

    #[test]
    fn test_guidance_verify_optional_session_id() {
        let registry = ToolRegistry::new();
        let schema = &registry.get("guidance_verify").unwrap().input_schema;
        assert!(schema.get("required").is_none());
        assert!(schema["properties"]["session_id"].is_object());
    }

    #[test]
    fn test_work_close_call_returns_status() {
        let registry = ToolRegistry::new();
        let result = registry.call("work_close", &json!({"id": "test-id"}));
        assert!(result.get("status").is_some());
    }

    #[test]
    fn test_work_sync_call_returns_status() {
        let registry = ToolRegistry::new();
        let result = registry.call("work_sync", &json!({}));
        assert!(result.get("status").is_some());
    }

    #[test]
    fn test_work_load_call_returns_status() {
        let registry = ToolRegistry::new();
        let result = registry.call("work_load", &json!({}));
        assert!(result.get("status").is_some());
    }

    #[test]
    fn test_guidance_verify_call_returns_status() {
        let registry = ToolRegistry::new();
        let result = registry.call("guidance_verify", &json!({}));
        assert!(result.get("status").is_some());
    }

    #[test]
    fn test_work_stealable_call_returns_status() {
        let registry = ToolRegistry::new();
        let result = registry.call("work_stealable", &json!({}));
        assert!(result.get("status").is_some());
    }

    #[test]
    fn test_work_status_call_returns_status() {
        let registry = ToolRegistry::new();
        let result = registry.call("work_status", &json!({}));
        assert!(result.get("status").is_some());
    }

    #[test]
    fn test_work_stealable_no_required_params() {
        let registry = ToolRegistry::new();
        let schema = &registry.get("work_stealable").unwrap().input_schema;
        assert!(schema.get("required").is_none());
    }

    #[test]
    fn test_work_status_no_required_params() {
        let registry = ToolRegistry::new();
        let schema = &registry.get("work_status").unwrap().input_schema;
        assert!(schema.get("required").is_none());
    }

    // ── ToolBuilder tests ──

    #[test]
    fn test_tool_builder_required_str() {
        let mut tools = HashMap::new();
        tools
            .tool("test_tool", "A test tool")
            .required_str("name", "The name")
            .build();
        let tool = tools.get("test_tool").unwrap();
        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, "A test tool");
        assert_eq!(tool.input_schema["type"], "object");
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("name")));
        assert_eq!(tool.input_schema["properties"]["name"]["type"], "string");
    }

    #[test]
    fn test_tool_builder_optional_fields() {
        let mut tools = HashMap::new();
        tools
            .tool("opt_tool", "Optional fields")
            .optional_str("filter", "Filter query")
            .optional_int("limit", "Max results")
            .optional_num("threshold", "Min threshold")
            .build();
        let tool = tools.get("opt_tool").unwrap();
        // No required array since all fields are optional
        assert!(tool.input_schema.get("required").is_none());
        assert_eq!(tool.input_schema["properties"]["filter"]["type"], "string");
        assert_eq!(tool.input_schema["properties"]["limit"]["type"], "integer");
        assert_eq!(
            tool.input_schema["properties"]["threshold"]["type"],
            "number"
        );
    }

    #[test]
    fn test_tool_builder_defaults() {
        let mut tools = HashMap::new();
        tools
            .tool("def_tool", "Defaults")
            .optional_int_default("limit", "Max results", 50)
            .optional_str_default("format", "Output format", "json")
            .optional_num_default("threshold", "Min threshold", 0.5)
            .build();
        let tool = tools.get("def_tool").unwrap();
        assert_eq!(tool.input_schema["properties"]["limit"]["default"], 50);
        assert_eq!(tool.input_schema["properties"]["format"]["default"], "json");
        assert_eq!(tool.input_schema["properties"]["threshold"]["default"], 0.5);
    }

    #[test]
    fn test_tool_builder_mixed_required_optional() {
        let mut tools = HashMap::new();
        tools
            .tool("mixed", "Mixed params")
            .required_str("id", "Item ID")
            .optional_str("description", "Optional desc")
            .required_bool("confirm", "Must confirm")
            .build();
        let tool = tools.get("mixed").unwrap();
        let required = tool.input_schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.iter().any(|v| v.as_str() == Some("id")));
        assert!(required.iter().any(|v| v.as_str() == Some("confirm")));
        assert!(!required.iter().any(|v| v.as_str() == Some("description")));
    }

    // ── ParamExt tests ──

    #[test]
    fn test_param_ext_str_or() {
        let params = json!({"name": "test", "empty": ""});
        assert_eq!(params.str_or("name", "default"), "test");
        assert_eq!(params.str_or("missing", "default"), "default");
        assert_eq!(params.str_or("empty", "default"), "");
    }

    #[test]
    fn test_param_ext_opt_str() {
        let params = json!({"name": "test"});
        assert_eq!(params.opt_str("name"), Some("test"));
        assert_eq!(params.opt_str("missing"), None);
    }

    #[test]
    fn test_param_ext_u64_or() {
        let params = json!({"count": 42});
        assert_eq!(params.u64_or("count", 0), 42);
        assert_eq!(params.u64_or("missing", 10), 10);
    }

    #[test]
    fn test_param_ext_i64_or() {
        let params = json!({"offset": -5});
        assert_eq!(params.i64_or("offset", 0), -5);
        assert_eq!(params.i64_or("missing", 100), 100);
    }

    #[test]
    fn test_param_ext_bool_or() {
        let params = json!({"flag": true});
        assert!(params.bool_or("flag", false));
        assert!(!params.bool_or("missing", false));
    }

    #[test]
    fn test_param_ext_opt_i64() {
        let params = json!({"val": 99});
        assert_eq!(params.opt_i64("val"), Some(99));
        assert_eq!(params.opt_i64("missing"), None);
    }

    #[test]
    fn test_param_ext_opt_u32() {
        let params = json!({"val": 42});
        assert_eq!(params.opt_u32("val"), Some(42));
        assert_eq!(params.opt_u32("missing"), None);
    }

    // ── Tool registration completeness ──

    #[test]
    fn test_all_work_tools_registered() {
        let registry = ToolRegistry::new();
        let work_tools = [
            "work_create",
            "work_list",
            "work_update",
            "work_log",
            "work_claim",
            "work_release",
            "work_steal",
            "work_heartbeat",
            "work_close",
            "work_sync",
            "work_load",
            "work_stealable",
            "work_status",
        ];
        for name in &work_tools {
            assert!(registry.get(name).is_some(), "Missing work tool: {name}");
        }
    }

    #[test]
    fn test_all_guidance_tools_registered() {
        let registry = ToolRegistry::new();
        for name in [
            "guidance_rules",
            "guidance_trust",
            "guidance_audit",
            "guidance_verify",
        ] {
            assert!(
                registry.get(name).is_some(),
                "Missing guidance tool: {name}"
            );
        }
    }

    #[test]
    fn test_all_trajectory_tools_registered() {
        let registry = ToolRegistry::new();
        for name in ["trajectory_list", "trajectory_get", "trajectory_judge"] {
            assert!(
                registry.get(name).is_some(),
                "Missing trajectory tool: {name}"
            );
        }
    }

    #[test]
    fn test_all_memory_tools_registered() {
        let registry = ToolRegistry::new();
        for name in [
            "memory_get",
            "memory_set",
            "memory_search",
            "memory_delete",
            "memory_list",
            "memory_import",
        ] {
            assert!(registry.get(name).is_some(), "Missing memory tool: {name}");
        }
    }
}
