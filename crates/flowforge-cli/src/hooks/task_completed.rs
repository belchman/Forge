use flowforge_core::hook::{self, TaskCompletedInput};
use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;

pub fn run() -> Result<()> {
    let input: TaskCompletedInput = hook::parse_stdin()?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    // Update routing weights based on task completion
    if config.hooks.learning {
        if let (Some(subject), Some(teammate)) = (&input.task_subject, &input.teammate_name) {
            update_routing_weight(&config, subject, teammate)?;
        }
    }

    Ok(())
}

fn update_routing_weight(config: &FlowForgeConfig, task_subject: &str, agent_name: &str) -> Result<()> {
    let db_path = config.db_path();
    if !db_path.exists() {
        return Ok(());
    }

    let db = MemoryDb::open(&db_path)?;

    // Extract a simple task pattern from the subject
    let task_pattern = task_subject
        .to_lowercase()
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");

    // Record a success for this agent on this task pattern
    db.record_routing_success(&task_pattern, agent_name)?;

    Ok(())
}
