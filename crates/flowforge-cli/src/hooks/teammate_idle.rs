use flowforge_core::hook::{self, TeammateIdleInput};
use flowforge_core::{FlowForgeConfig, Result, TeamMemberStatus};
use flowforge_tmux::TmuxStateManager;

pub fn run() -> Result<()> {
    let input: TeammateIdleInput = hook::parse_stdin()?;

    // Update tmux state
    let state_mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    let _ = state_mgr.update_member_status(
        &input.teammate_name,
        TeamMemberStatus::Idle,
        None,
    );
    let _ = state_mgr.add_event(format!("{} went idle", input.teammate_name));

    Ok(())
}
