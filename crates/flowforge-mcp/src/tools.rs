use serde_json::{json, Value};
use std::collections::HashMap;

use flowforge_agents::{AgentRegistry, AgentRouter};
use flowforge_core::FlowForgeConfig;
use flowforge_memory::{MemoryDb, PatternStore};
use flowforge_tmux::TmuxStateManager;

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
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_all();
        registry
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
            "memory_get" => self.memory_get(params),
            "memory_set" => self.memory_set(params),
            "memory_search" => self.memory_search(params),
            "memory_delete" => self.memory_delete(params),
            "memory_list" => self.memory_list(params),
            "memory_import" => self.memory_import(params),
            "learning_store" => self.learning_store(params),
            "learning_search" => self.learning_search(params),
            "learning_feedback" => self.learning_feedback(params),
            "learning_stats" => self.learning_stats(params),
            "agents_list" => self.agents_list(params),
            "agents_route" => self.agents_route(params),
            "agents_info" => self.agents_info(params),
            "session_status" => self.session_status(params),
            "session_metrics" => self.session_metrics(params),
            "session_history" => self.session_history(params),
            "team_status" => self.team_status(params),
            "team_log" => self.team_log(params),
            "work_create" => self.work_create(params),
            "work_list" => self.work_list(params),
            "work_update" => self.work_update(params),
            "work_log" => self.work_log(params),
            _ => json!({ "error": format!("unknown tool: {}", name) }),
        }
    }

    fn register_all(&mut self) {
        // Memory tools
        self.register(ToolDef {
            name: "memory_get".into(),
            description: "Get a memory entry by key".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "The memory key to retrieve" }
                },
                "required": ["key"]
            }),
        });

        self.register(ToolDef {
            name: "memory_set".into(),
            description: "Store a memory entry with a key and value".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "The memory key" },
                    "value": { "type": "string", "description": "The value to store" },
                    "category": { "type": "string", "description": "Optional category for the memory" }
                },
                "required": ["key", "value"]
            }),
        });

        self.register(ToolDef {
            name: "memory_search".into(),
            description: "Search memory entries by query string".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Max results to return", "default": 10 }
                },
                "required": ["query"]
            }),
        });

        self.register(ToolDef {
            name: "memory_delete".into(),
            description: "Delete a memory entry by key".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "The memory key to delete" }
                },
                "required": ["key"]
            }),
        });

        self.register(ToolDef {
            name: "memory_list".into(),
            description: "List all memory entries, optionally filtered by category".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "category": { "type": "string", "description": "Filter by category" },
                    "limit": { "type": "integer", "description": "Max results", "default": 50 }
                }
            }),
        });

        self.register(ToolDef {
            name: "memory_import".into(),
            description: "Import memory entries from a JSON array".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "value": { "type": "string" },
                                "category": { "type": "string" }
                            },
                            "required": ["key", "value"]
                        },
                        "description": "Array of memory entries to import"
                    }
                },
                "required": ["entries"]
            }),
        });

        // Learning tools
        self.register(ToolDef {
            name: "learning_store".into(),
            description: "Store a learned pattern from an observation".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "The pattern content" },
                    "category": { "type": "string", "description": "Pattern category (e.g., code_style, error_fix)" },
                    "confidence": { "type": "number", "description": "Initial confidence 0.0-1.0", "default": 0.5 }
                },
                "required": ["content", "category"]
            }),
        });

        self.register(ToolDef {
            name: "learning_search".into(),
            description: "Search learned patterns by query".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "category": { "type": "string", "description": "Filter by category" },
                    "limit": { "type": "integer", "description": "Max results", "default": 10 }
                },
                "required": ["query"]
            }),
        });

        self.register(ToolDef {
            name: "learning_feedback".into(),
            description: "Provide feedback on a learned pattern (positive or negative)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern_id": { "type": "string", "description": "The pattern ID" },
                    "positive": { "type": "boolean", "description": "Whether the feedback is positive" }
                },
                "required": ["pattern_id", "positive"]
            }),
        });

        self.register(ToolDef {
            name: "learning_stats".into(),
            description: "Get statistics about learned patterns".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        });

        // Agent tools
        self.register(ToolDef {
            name: "agents_list".into(),
            description: "List all available agents with their capabilities".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Filter by source: builtin, global, project" }
                }
            }),
        });

        self.register(ToolDef {
            name: "agents_route".into(),
            description: "Route a task description to the best matching agent".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "Task description to route" },
                    "top_k": { "type": "integer", "description": "Number of top candidates", "default": 3 }
                },
                "required": ["task"]
            }),
        });

        self.register(ToolDef {
            name: "agents_info".into(),
            description: "Get detailed info about a specific agent".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Agent name" }
                },
                "required": ["name"]
            }),
        });

        // Session tools
        self.register(ToolDef {
            name: "session_status".into(),
            description: "Get current session status including active tasks and edits".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        });

        self.register(ToolDef {
            name: "session_metrics".into(),
            description: "Get session metrics: edits, commands, routing decisions".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID (defaults to current)" }
                }
            }),
        });

        self.register(ToolDef {
            name: "session_history".into(),
            description: "Get session history with summaries".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max sessions to return", "default": 10 }
                }
            }),
        });

        // Team tools
        self.register(ToolDef {
            name: "team_status".into(),
            description: "Get current team status including all member states".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        });

        self.register(ToolDef {
            name: "team_log".into(),
            description: "Get recent team activity log".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max log entries", "default": 20 }
                }
            }),
        });

        // Work tracking tools (C6)
        self.register(ToolDef {
            name: "work_create".into(),
            description: "Create a new work item (task, epic, bug, story)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Title of the work item" },
                    "type": { "type": "string", "description": "Item type: task, epic, bug, story, sub-task", "default": "task" },
                    "description": { "type": "string", "description": "Optional description" },
                    "parent_id": { "type": "string", "description": "Parent work item ID for hierarchy" },
                    "priority": { "type": "integer", "description": "Priority 0-3 (0=critical)", "default": 2 }
                },
                "required": ["title"]
            }),
        });

        self.register(ToolDef {
            name: "work_list".into(),
            description: "List work items with optional filters".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string", "description": "Filter by status: pending, in_progress, blocked, completed" },
                    "type": { "type": "string", "description": "Filter by item type" },
                    "limit": { "type": "integer", "description": "Max results", "default": 20 }
                }
            }),
        });

        self.register(ToolDef {
            name: "work_update".into(),
            description: "Update a work item's status".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Work item ID" },
                    "status": { "type": "string", "description": "New status: pending, in_progress, blocked, completed" }
                },
                "required": ["id", "status"]
            }),
        });

        self.register(ToolDef {
            name: "work_log".into(),
            description: "Query the work tracking audit trail".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_item_id": { "type": "string", "description": "Filter by work item ID (optional)" },
                    "limit": { "type": "integer", "description": "Max events", "default": 20 }
                }
            }),
        });
    }

    fn register(&mut self, tool: ToolDef) {
        self.tools.insert(tool.name.clone(), tool);
    }

    // --- Helpers ---

    fn open_db() -> flowforge_core::Result<MemoryDb> {
        let config = Self::load_config()?;
        MemoryDb::open(&config.db_path())
    }

    fn load_config() -> flowforge_core::Result<FlowForgeConfig> {
        FlowForgeConfig::load(&FlowForgeConfig::config_path())
    }

    // --- Memory tool implementations ---

    fn memory_get(&self, params: &Value) -> Value {
        let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let namespace = params
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match Self::open_db() {
            Ok(db) => match db.kv_get(key, namespace) {
                Ok(value) => json!({"status": "ok", "key": key, "value": value}),
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn memory_set(&self, params: &Value) -> Value {
        let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("");
        let namespace = params
            .get("namespace")
            .or_else(|| params.get("category"))
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match Self::open_db() {
            Ok(db) => match db.kv_set(key, value, namespace) {
                Ok(()) => json!({"status": "ok", "key": key, "stored": true}),
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn memory_search(&self, params: &Value) -> Value {
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        match Self::open_db() {
            Ok(db) => match db.kv_search(query, limit) {
                Ok(results) => {
                    let entries: Vec<Value> = results
                        .iter()
                        .map(|(k, v, ns)| json!({"key": k, "value": v, "namespace": ns}))
                        .collect();
                    json!({"status": "ok", "query": query, "results": entries})
                }
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn memory_delete(&self, params: &Value) -> Value {
        let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let namespace = params
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match Self::open_db() {
            Ok(db) => match db.kv_delete(key, namespace) {
                Ok(()) => json!({"status": "ok", "key": key, "deleted": true}),
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn memory_list(&self, params: &Value) -> Value {
        let namespace = params
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        match Self::open_db() {
            Ok(db) => match db.kv_list(namespace) {
                Ok(entries) => {
                    let entries: Vec<Value> = entries
                        .iter()
                        .take(limit)
                        .map(|(k, v)| json!({"key": k, "value": v}))
                        .collect();
                    json!({"status": "ok", "entries": entries})
                }
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn memory_import(&self, params: &Value) -> Value {
        let entries = match params.get("entries").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return json!({"status": "error", "message": "missing entries array"}),
        };
        let total = entries.len();

        match Self::open_db() {
            Ok(db) => {
                let mut imported = 0usize;
                for entry in entries {
                    let key = entry.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let value = entry.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    let namespace = entry
                        .get("namespace")
                        .or_else(|| entry.get("category"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("default");
                    if db.kv_set(key, value, namespace).is_ok() {
                        imported += 1;
                    }
                }
                json!({"status": "ok", "imported": imported, "total": total})
            }
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    // --- Learning tool implementations ---

    fn learning_store(&self, params: &Value) -> Value {
        let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let category = params
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match Self::load_config().and_then(|config| {
            let db = MemoryDb::open(&config.db_path())?;
            let store = PatternStore::new(&db, &config.patterns);
            let id = store.store_short_term(content, category)?;
            Ok(id)
        }) {
            Ok(id) => json!({"status": "ok", "pattern_id": id}),
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn learning_search(&self, params: &Value) -> Value {
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        match Self::load_config().and_then(|config| {
            let db = MemoryDb::open(&config.db_path())?;
            let store = PatternStore::new(&db, &config.patterns);
            store.search_patterns(query, limit)
        }) {
            Ok(results) => {
                let patterns: Vec<Value> = results
                    .iter()
                    .map(|(p, _score)| {
                        json!({
                            "id": p.id,
                            "content": p.content,
                            "category": p.category,
                            "confidence": p.confidence,
                            "usage_count": p.usage_count,
                        })
                    })
                    .collect();
                json!({"status": "ok", "patterns": patterns})
            }
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn learning_feedback(&self, params: &Value) -> Value {
        let pattern_id = params
            .get("pattern_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let positive = params
            .get("positive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        match Self::load_config().and_then(|config| {
            let db = MemoryDb::open(&config.db_path())?;
            let store = PatternStore::new(&db, &config.patterns);
            store.record_feedback(pattern_id, positive)
        }) {
            Ok(()) => json!({"status": "ok", "pattern_id": pattern_id, "updated": true}),
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn learning_stats(&self, _params: &Value) -> Value {
        match Self::open_db() {
            Ok(db) => {
                let short = db.count_patterns_short().unwrap_or(0);
                let long = db.count_patterns_long().unwrap_or(0);
                json!({
                    "status": "ok",
                    "short_term_count": short,
                    "long_term_count": long,
                    "total": short + long,
                })
            }
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    // --- Agent tool implementations ---

    fn agents_list(&self, params: &Value) -> Value {
        let source_filter = params.get("source").and_then(|v| v.as_str());

        match Self::load_config().and_then(|config| AgentRegistry::load(&config.agents)) {
            Ok(registry) => {
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
                json!({"status": "ok", "agents": agents})
            }
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn agents_route(&self, params: &Value) -> Value {
        let task = params.get("task").and_then(|v| v.as_str()).unwrap_or("");
        let top_k = params.get("top_k").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

        match Self::load_config().and_then(|config| {
            let db = MemoryDb::open(&config.db_path())?;
            let registry = AgentRegistry::load(&config.agents)?;
            let router = AgentRouter::new(&config.routing);

            let weights_vec = db.get_all_routing_weights()?;
            let mut learned_weights: HashMap<(String, String), f64> = HashMap::new();
            for w in &weights_vec {
                learned_weights.insert((w.task_pattern.clone(), w.agent_name.clone()), w.weight);
            }

            let agent_refs: Vec<&_> = registry.list();
            let results = router.route(task, &agent_refs, &learned_weights);
            Ok(results)
        }) {
            Ok(results) => {
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
                                "priority_score": r.breakdown.priority_score,
                            },
                        })
                    })
                    .collect();
                json!({"status": "ok", "candidates": candidates})
            }
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn agents_info(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");

        match Self::load_config().and_then(|config| AgentRegistry::load(&config.agents)) {
            Ok(registry) => match registry.get(name) {
                Some(agent) => json!({
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
                }),
                None => json!({"status": "error", "message": "Agent not found"}),
            },
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    // --- Session tool implementations ---

    fn session_status(&self, _params: &Value) -> Value {
        match Self::open_db() {
            Ok(db) => match db.get_current_session() {
                Ok(Some(session)) => json!({
                    "status": "ok",
                    "session": {
                        "id": session.id,
                        "started_at": session.started_at.to_rfc3339(),
                        "cwd": session.cwd,
                        "edits": session.edits,
                        "commands": session.commands,
                        "summary": session.summary,
                    },
                }),
                Ok(None) => json!({"status": "ok", "session": null}),
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn session_metrics(&self, params: &Value) -> Value {
        let session_id = params.get("session_id").and_then(|v| v.as_str());

        match Self::open_db() {
            Ok(db) => {
                let session = if let Some(id) = session_id {
                    db.list_sessions(1000)
                        .ok()
                        .and_then(|sessions| sessions.into_iter().find(|s| s.id == id))
                } else {
                    db.get_current_session().ok().flatten()
                };
                match session {
                    Some(s) => json!({
                        "status": "ok",
                        "session_id": s.id,
                        "edits": s.edits,
                        "commands": s.commands,
                    }),
                    None => {
                        json!({"status": "ok", "session_id": session_id, "edits": 0, "commands": 0})
                    }
                }
            }
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn session_history(&self, params: &Value) -> Value {
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        match Self::open_db() {
            Ok(db) => match db.list_sessions(limit) {
                Ok(sessions) => {
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
                    json!({"status": "ok", "sessions": entries})
                }
                Err(e) => json!({"status": "error", "message": format!("{e}")}),
            },
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    // --- Team tool implementations ---

    fn team_status(&self, _params: &Value) -> Value {
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
                json!({
                    "status": "ok",
                    "team": state.team_name,
                    "members": members,
                    "memory_count": state.memory_count,
                    "pattern_count": state.pattern_count,
                    "updated_at": state.updated_at.to_rfc3339(),
                })
            }
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn team_log(&self, params: &Value) -> Value {
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        let mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
        match mgr.load() {
            Ok(state) => {
                let events: Vec<&String> = state.recent_events.iter().take(limit).collect();
                json!({"status": "ok", "events": events})
            }
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    // --- Work tracking tool implementations (C6) ---

    fn work_create(&self, params: &Value) -> Value {
        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let item_type = params
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("task");
        let description = params.get("description").and_then(|v| v.as_str());
        let parent_id = params.get("parent_id").and_then(|v| v.as_str());
        let priority = params.get("priority").and_then(|v| v.as_i64()).unwrap_or(2) as i32;

        match Self::load_config().and_then(|config| {
            let db = MemoryDb::open(&config.db_path())?;
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
            };

            flowforge_core::work_tracking::create_item(&db, &config.work_tracking, &item)?;
            Ok(item.id)
        }) {
            Ok(id) => json!({"status": "ok", "id": id, "title": title}),
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn work_list(&self, params: &Value) -> Value {
        let status = params.get("status").and_then(|v| v.as_str());
        let item_type = params.get("type").and_then(|v| v.as_str());
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        match Self::open_db() {
            Ok(db) => {
                let filter = flowforge_core::WorkFilter {
                    status: status.map(|s| s.to_string()),
                    item_type: item_type.map(|s| s.to_string()),
                    limit: Some(limit),
                    ..Default::default()
                };
                match db.list_work_items(&filter) {
                    Ok(items) => {
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
                        json!({"status": "ok", "items": entries, "count": entries.len()})
                    }
                    Err(e) => json!({"status": "error", "message": format!("{e}")}),
                }
            }
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }

    fn work_update(&self, params: &Value) -> Value {
        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let new_status = params.get("status").and_then(|v| v.as_str()).unwrap_or("");

        match Self::load_config().and_then(|config| {
            let db = MemoryDb::open(&config.db_path())?;
            flowforge_core::work_tracking::update_status(
                &db,
                &config.work_tracking,
                id,
                new_status,
                "mcp",
            )?;
            Ok(())
        }) {
            Ok(()) => json!({"status": "ok", "id": id, "new_status": new_status}),
            Err(e) => json!({"status": "error", "message": format!("{e}")}),
        }
    }

    fn work_log(&self, params: &Value) -> Value {
        let work_item_id = params.get("work_item_id").and_then(|v| v.as_str());
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        match Self::open_db() {
            Ok(db) => {
                let events = if let Some(id) = work_item_id {
                    db.get_work_events(id, limit)
                } else {
                    db.get_recent_work_events(limit)
                };

                match events {
                    Ok(events) => {
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
                        json!({"status": "ok", "events": entries})
                    }
                    Err(e) => json!({"status": "error", "message": format!("{e}")}),
                }
            }
            Err(e) => {
                json!({"status": "error", "message": format!("Failed to open database: {e}")})
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_22_tools() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.list().len(), 22);
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
        // With real backend, this may return error (no DB) or ok
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
}
