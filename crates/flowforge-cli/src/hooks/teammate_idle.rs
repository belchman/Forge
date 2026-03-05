use flowforge_core::hook::TeammateIdleInput;
use flowforge_core::{AgentSessionStatus, FlowForgeConfig, Result, TeamMemberStatus};
use flowforge_tmux::TmuxStateManager;

pub fn run() -> Result<()> {
    let ctx = super::HookContext::init()?;
    let input = TeammateIdleInput::from_value(&ctx.raw)?;

    let teammate_name = input.teammate_name.as_deref().unwrap_or("unknown");

    // Update tmux state
    let state_mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    let _ = state_mgr.update_member_status(teammate_name, TeamMemberStatus::Idle, None);
    let _ = state_mgr.add_event(format!("{} went idle", teammate_name));

    // Persist idle status to DB and detect stale work items
    ctx.with_db("teammate_idle", |db| {
        db.update_agent_session_status(teammate_name, AgentSessionStatus::Idle)?;

        if ctx.config.work_tracking.work_stealing.enabled {
            flowforge_core::work_tracking::detect_stale(db, &ctx.config.work_tracking)?;
        }
        Ok(())
    });

    Ok(())
}
