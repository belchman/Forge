use flowforge_core::hook::{self, ContextOutput, SubagentStartInput};
use flowforge_core::FlowForgeConfig;
use flowforge_tmux::TmuxStateManager;

pub fn run() -> flowforge_core::Result<()> {
    let input: SubagentStartInput = hook::parse_stdin()?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    // Update tmux state
    let state_mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    let _ = state_mgr.add_member(
        &input.agent_id,
        input.agent_type.as_deref().unwrap_or("general"),
    );
    let _ = state_mgr.add_event(format!(
        "{} started ({})",
        input.agent_id,
        input.agent_type.as_deref().unwrap_or("general")
    ));

    // Inject agent-specific context if we have an agent type match
    let mut context_parts = Vec::new();

    if let Some(agent_type) = &input.agent_type {
        if let Ok(registry) = flowforge_agents::AgentRegistry::load(&config.agents) {
            if let Some(agent) = registry.get(agent_type) {
                if !agent.body.is_empty() {
                    context_parts.push(format!(
                        "[FlowForge] Agent guidance for {}:\n{}",
                        agent.name, agent.body
                    ));
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
