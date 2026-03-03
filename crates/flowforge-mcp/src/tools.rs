use serde_json::{json, Value};
use std::collections::HashMap;

pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct ToolRegistry {
    tools: HashMap<String, ToolDef>,
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
    }

    fn register(&mut self, tool: ToolDef) {
        self.tools.insert(tool.name.clone(), tool);
    }

    // --- Tool implementations ---
    // These return structured responses and will be wired to actual
    // flowforge-memory/agents backends once those crates are fully implemented.

    fn memory_get(&self, params: &Value) -> Value {
        let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "key": key,
            "value": null,
            "message": "memory backend not yet connected"
        })
    }

    fn memory_set(&self, params: &Value) -> Value {
        let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "key": key,
            "value": value,
            "message": "memory backend not yet connected"
        })
    }

    fn memory_search(&self, params: &Value) -> Value {
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "query": query,
            "results": [],
            "message": "memory backend not yet connected"
        })
    }

    fn memory_delete(&self, params: &Value) -> Value {
        let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "key": key,
            "deleted": false,
            "message": "memory backend not yet connected"
        })
    }

    fn memory_list(&self, params: &Value) -> Value {
        let category = params.get("category").and_then(|v| v.as_str());
        json!({
            "status": "ok",
            "category": category,
            "entries": [],
            "message": "memory backend not yet connected"
        })
    }

    fn memory_import(&self, params: &Value) -> Value {
        let count = params
            .get("entries")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        json!({
            "status": "ok",
            "imported": 0,
            "total": count,
            "message": "memory backend not yet connected"
        })
    }

    fn learning_store(&self, params: &Value) -> Value {
        let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let category = params.get("category").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "content": content,
            "category": category,
            "pattern_id": null,
            "message": "learning backend not yet connected"
        })
    }

    fn learning_search(&self, params: &Value) -> Value {
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "query": query,
            "patterns": [],
            "message": "learning backend not yet connected"
        })
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
        json!({
            "status": "ok",
            "pattern_id": pattern_id,
            "positive": positive,
            "message": "learning backend not yet connected"
        })
    }

    fn learning_stats(&self, _params: &Value) -> Value {
        json!({
            "status": "ok",
            "short_term_count": 0,
            "long_term_count": 0,
            "categories": {},
            "message": "learning backend not yet connected"
        })
    }

    fn agents_list(&self, params: &Value) -> Value {
        let source = params.get("source").and_then(|v| v.as_str());
        json!({
            "status": "ok",
            "source_filter": source,
            "agents": [],
            "message": "agents backend not yet connected"
        })
    }

    fn agents_route(&self, params: &Value) -> Value {
        let task = params.get("task").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "task": task,
            "candidates": [],
            "message": "agents backend not yet connected"
        })
    }

    fn agents_info(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        json!({
            "status": "ok",
            "name": name,
            "agent": null,
            "message": "agents backend not yet connected"
        })
    }

    fn session_status(&self, _params: &Value) -> Value {
        json!({
            "status": "ok",
            "session": null,
            "message": "session backend not yet connected"
        })
    }

    fn session_metrics(&self, params: &Value) -> Value {
        let session_id = params.get("session_id").and_then(|v| v.as_str());
        json!({
            "status": "ok",
            "session_id": session_id,
            "edits": 0,
            "commands": 0,
            "message": "session backend not yet connected"
        })
    }

    fn session_history(&self, params: &Value) -> Value {
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);
        json!({
            "status": "ok",
            "limit": limit,
            "sessions": [],
            "message": "session backend not yet connected"
        })
    }

    fn team_status(&self, _params: &Value) -> Value {
        json!({
            "status": "ok",
            "team": null,
            "members": [],
            "message": "team backend not yet connected"
        })
    }

    fn team_log(&self, params: &Value) -> Value {
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);
        json!({
            "status": "ok",
            "limit": limit,
            "events": [],
            "message": "team backend not yet connected"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_18_tools() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.list().len(), 18);
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
        assert_eq!(result["status"], "ok");
        assert_eq!(result["key"], "test");
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
