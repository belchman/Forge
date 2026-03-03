use flowforge_core::hook::{self, ContextOutput, UserPromptSubmitInput};
use flowforge_core::{FlowForgeConfig, Result};
use flowforge_agents::{AgentRegistry, AgentRouter};
use flowforge_memory::MemoryDb;
use std::collections::HashMap;

pub fn run() -> Result<()> {
    let input: UserPromptSubmitInput = hook::parse_stdin()?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    let mut context_parts: Vec<String> = Vec::new();

    // Route the task to suggested agents
    if config.hooks.routing {
        if let Ok(registry) = AgentRegistry::load(&config.agents) {
            let router = AgentRouter::new(&config.routing);
            let agents: Vec<&_> = registry.list().into_iter().collect();

            // Load learned weights from DB if available
            let learned_weights = load_learned_weights(&config);

            let results = router.route(&input.prompt, &agents, &learned_weights);

            if let Some(top) = results.first() {
                if top.confidence > 0.3 {
                    let mut routing_ctx = format!(
                        "[FlowForge] Suggested agent: {} (confidence: {:.0}%)",
                        top.agent_name,
                        top.confidence * 100.0
                    );

                    // Include agent body for top match
                    if let Some(agent) = registry.get(&top.agent_name) {
                        if !agent.body.is_empty() {
                            routing_ctx.push_str(&format!("\n\n{}", agent.body));
                        }
                    }

                    // Show runner-up if close
                    if results.len() > 1 && results[1].confidence > 0.25 {
                        routing_ctx.push_str(&format!(
                            "\nAlternative: {} ({:.0}%)",
                            results[1].agent_name,
                            results[1].confidence * 100.0
                        ));
                    }

                    context_parts.push(routing_ctx);
                }
            }
        }
    }

    // Search FlowForge memory for relevant patterns and stored knowledge
    if config.hooks.learning {
        if let Ok(db) = MemoryDb::open(&config.db_path()) {
            // Search learned patterns
            if let Ok(patterns) = db.search_patterns_short(&input.prompt, 3) {
                let relevant: Vec<_> = patterns
                    .into_iter()
                    .filter(|p| p.confidence > 0.5)
                    .collect();
                if !relevant.is_empty() {
                    let mut pattern_ctx = String::from("[FlowForge Memory] Relevant patterns:");
                    for p in &relevant {
                        pattern_ctx.push_str(&format!("\n- {} (conf: {:.0}%)", p.content, p.confidence * 100.0));
                    }
                    context_parts.push(pattern_ctx);
                }
            }

            // Also search long-term patterns for high-confidence knowledge
            if let Ok(long_patterns) = db.search_patterns_long(&input.prompt, 3) {
                let relevant: Vec<_> = long_patterns
                    .into_iter()
                    .filter(|p| p.confidence > 0.6)
                    .collect();
                if !relevant.is_empty() {
                    let mut lt_ctx = String::from("[FlowForge Memory] Proven patterns:");
                    for p in &relevant {
                        lt_ctx.push_str(&format!(
                            "\n- {} (conf: {:.0}%, used: {}x, success: {})",
                            p.content, p.confidence * 100.0, p.usage_count, p.success_count
                        ));
                    }
                    context_parts.push(lt_ctx);
                }
            }

            // Search key-value memory for relevant stored knowledge
            if let Ok(kv_results) = db.kv_search(&input.prompt, 3) {
                if !kv_results.is_empty() {
                    let mut kv_ctx = String::from("[FlowForge Memory] Stored knowledge:");
                    for (key, value, _ns) in &kv_results {
                        kv_ctx.push_str(&format!("\n- {}: {}", key, value));
                    }
                    context_parts.push(kv_ctx);
                }
            }
        }
    }

    if context_parts.is_empty() {
        let output = ContextOutput::none();
        hook::write_stdout(&output)?;
    } else {
        let output = ContextOutput::with_context(context_parts.join("\n\n"));
        hook::write_stdout(&output)?;
    }

    Ok(())
}

fn load_learned_weights(config: &FlowForgeConfig) -> HashMap<(String, String), f64> {
    let mut weights = HashMap::new();
    if let Ok(db) = MemoryDb::open(&config.db_path()) {
        if let Ok(all_weights) = db.get_all_routing_weights() {
            for w in all_weights {
                weights.insert((w.task_pattern, w.agent_name), w.weight);
            }
        }
    }
    weights
}
