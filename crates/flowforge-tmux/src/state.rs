use std::path::PathBuf;

use chrono::Utc;
use flowforge_core::{TeamMemberState, TeamMemberStatus, TmuxState};
use fs2::FileExt;

pub struct TmuxStateManager {
    state_path: PathBuf,
}

impl TmuxStateManager {
    pub fn new(state_path: PathBuf) -> Self {
        Self { state_path }
    }

    /// Lock the state file, load state, apply a mutation, save, and unlock.
    fn with_lock<F, T>(&self, f: F) -> flowforge_core::Result<T>
    where
        F: FnOnce(&mut TmuxState) -> flowforge_core::Result<T>,
    {
        if let Some(parent) = self.state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let lock_path = self.state_path.with_extension("lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&lock_path)?;
        lock_file
            .lock_exclusive()
            .map_err(|e| flowforge_core::Error::Tmux(format!("Failed to acquire lock: {e}")))?;

        let result = (|| {
            let mut state = self.load_inner()?;
            let result = f(&mut state)?;
            self.save_inner(&state)?;
            Ok(result)
        })();

        // Always unlock, even on error
        let _ = FileExt::unlock(&lock_file);
        result
    }

    fn load_inner(&self) -> flowforge_core::Result<TmuxState> {
        if self.state_path.exists() {
            let content = std::fs::read_to_string(&self.state_path)?;
            let state: TmuxState = serde_json::from_str(&content)?;
            Ok(state)
        } else {
            Ok(TmuxState {
                session_name: "flowforge".to_string(),
                team_name: None,
                members: Vec::new(),
                recent_events: Vec::new(),
                memory_count: 0,
                pattern_count: 0,
                updated_at: Utc::now(),
            })
        }
    }

    fn save_inner(&self, state: &TmuxState) -> flowforge_core::Result<()> {
        if let Some(parent) = self.state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(state)?;
        std::fs::write(&self.state_path, content)?;
        Ok(())
    }

    pub fn load(&self) -> flowforge_core::Result<TmuxState> {
        self.load_inner()
    }

    pub fn save(&self, state: &TmuxState) -> flowforge_core::Result<()> {
        self.save_inner(state)
    }

    pub fn add_member(&self, agent_id: &str, agent_type: &str) -> flowforge_core::Result<()> {
        let agent_id = agent_id.to_string();
        let agent_type = agent_type.to_string();
        self.with_lock(|state| {
            if state.members.iter().any(|m| m.agent_id == agent_id) {
                return Ok(());
            }
            state.members.push(TeamMemberState {
                agent_id: agent_id.clone(),
                agent_type,
                status: TeamMemberStatus::Idle,
                current_task: None,
                updated_at: Utc::now(),
            });
            state.updated_at = Utc::now();
            Ok(())
        })
    }

    pub fn update_member_status(
        &self,
        agent_id: &str,
        status: TeamMemberStatus,
        task: Option<String>,
    ) -> flowforge_core::Result<()> {
        let agent_id = agent_id.to_string();
        self.with_lock(|state| {
            if let Some(member) = state.members.iter_mut().find(|m| m.agent_id == agent_id) {
                member.status = status;
                member.current_task = task;
                member.updated_at = Utc::now();
            }
            state.updated_at = Utc::now();
            Ok(())
        })
    }

    pub fn remove_member(&self, agent_id: &str) -> flowforge_core::Result<()> {
        let agent_id = agent_id.to_string();
        self.with_lock(|state| {
            state.members.retain(|m| m.agent_id != agent_id);
            state.updated_at = Utc::now();
            Ok(())
        })
    }

    pub fn add_event(&self, event: String) -> flowforge_core::Result<()> {
        self.with_lock(|state| {
            state.recent_events.push(event);
            if state.recent_events.len() > 20 {
                let start = state.recent_events.len() - 20;
                state.recent_events = state.recent_events[start..].to_vec();
            }
            state.updated_at = Utc::now();
            Ok(())
        })
    }

    pub fn update_counts(
        &self,
        memory_count: u64,
        pattern_count: u64,
    ) -> flowforge_core::Result<()> {
        self.with_lock(|state| {
            state.memory_count = memory_count;
            state.pattern_count = pattern_count;
            state.updated_at = Utc::now();
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_default_when_missing() {
        let path = PathBuf::from("/tmp/flowforge-test-state-missing.json");
        let _ = fs::remove_file(&path);
        let mgr = TmuxStateManager::new(path);
        let state = mgr.load().unwrap();
        assert_eq!(state.session_name, "flowforge");
        assert!(state.members.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let path = PathBuf::from("/tmp/flowforge-test-state-roundtrip.json");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
        let mgr = TmuxStateManager::new(path.clone());

        mgr.add_member("lead", "team-lead").unwrap();
        mgr.add_member("dev", "implementer").unwrap();

        let state = mgr.load().unwrap();
        assert_eq!(state.members.len(), 2);
        assert_eq!(state.members[0].agent_id, "lead");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn test_update_member_status() {
        let path = PathBuf::from("/tmp/flowforge-test-state-status.json");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
        let mgr = TmuxStateManager::new(path.clone());

        mgr.add_member("dev", "implementer").unwrap();
        mgr.update_member_status("dev", TeamMemberStatus::Active, Some("Auth module".into()))
            .unwrap();

        let state = mgr.load().unwrap();
        assert_eq!(state.members[0].status, TeamMemberStatus::Active);
        assert_eq!(
            state.members[0].current_task.as_deref(),
            Some("Auth module")
        );

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn test_remove_member() {
        let path = PathBuf::from("/tmp/flowforge-test-state-remove.json");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
        let mgr = TmuxStateManager::new(path.clone());

        mgr.add_member("a", "type-a").unwrap();
        mgr.add_member("b", "type-b").unwrap();
        mgr.remove_member("a").unwrap();

        let state = mgr.load().unwrap();
        assert_eq!(state.members.len(), 1);
        assert_eq!(state.members[0].agent_id, "b");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn test_events_capped_at_20() {
        let path = PathBuf::from("/tmp/flowforge-test-state-events.json");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
        let mgr = TmuxStateManager::new(path.clone());

        for i in 0..25 {
            mgr.add_event(format!("event-{}", i)).unwrap();
        }

        let state = mgr.load().unwrap();
        assert_eq!(state.recent_events.len(), 20);
        assert_eq!(state.recent_events[0], "event-5");
        assert_eq!(state.recent_events[19], "event-24");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }
}
