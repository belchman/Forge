use flowforge_core::hook::{self, SubagentStopInput};
use flowforge_core::{FlowForgeConfig, Result, TeamMemberStatus};
use flowforge_memory::MemoryDb;
use flowforge_tmux::TmuxStateManager;

pub fn run() -> Result<()> {
    let input: SubagentStopInput = hook::parse_stdin()?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    // Update tmux state
    let state_mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    let _ = state_mgr.update_member_status(
        &input.agent_id,
        TeamMemberStatus::Completed,
        None,
    );
    let _ = state_mgr.add_event(format!("{} stopped", input.agent_id));

    // Extract patterns from agent output if learning is enabled
    if config.hooks.learning {
        if let Some(message) = &input.last_assistant_message {
            extract_patterns(&config, message)?;
        }
    }

    Ok(())
}

fn extract_patterns(config: &FlowForgeConfig, message: &str) -> Result<()> {
    let db_path = config.db_path();
    if !db_path.exists() {
        return Ok(());
    }

    let db = MemoryDb::open(&db_path)?;
    let store = flowforge_memory::PatternStore::new(&db, &config.patterns);

    for line in message.lines() {
        let trimmed = line.trim();

        if trimmed.len() < 20 || trimmed.len() > 200 {
            continue;
        }

        if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("Note:")
            || trimmed.starts_with("Pattern:")
            || trimmed.starts_with("Learned:")
        {
            let content = trimmed
                .trim_start_matches("- ")
                .trim_start_matches("* ")
                .trim_start_matches("Note: ")
                .trim_start_matches("Pattern: ")
                .trim_start_matches("Learned: ");

            if !content.is_empty() {
                let _ = store.store_short_term(content, "agent-output");
            }
        }
    }

    Ok(())
}
