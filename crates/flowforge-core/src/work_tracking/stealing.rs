//! Work-stealing functions: claim, release, steal, and stale detection.

use crate::config::WorkTrackingConfig;
use crate::types::WorkItem;
use crate::Result;

use super::WorkStealing;

/// Claim a work item for a session.
pub fn claim_item(db: &dyn WorkStealing, id: &str, session_id: &str) -> Result<bool> {
    db.claim_work_item(id, session_id)
}

/// Release a claimed work item.
pub fn release_item(db: &dyn WorkStealing, id: &str) -> Result<()> {
    db.release_work_item(id)
}

/// Steal a stealable work item for a new session.
pub fn steal_item(db: &dyn WorkStealing, id: &str, new_session_id: &str) -> Result<bool> {
    db.steal_work_item(id, new_session_id)
}

/// Detect and mark stale items, auto-release abandoned ones.
pub fn detect_stale(db: &dyn WorkStealing, config: &WorkTrackingConfig) -> Result<(u64, u64)> {
    let ws = &config.work_stealing;
    if !ws.enabled {
        return Ok((0, 0));
    }
    let marked = db.mark_stale_items_stealable(ws.stale_threshold_mins, ws.stale_min_progress)?;
    let released = db.auto_release_abandoned(ws.abandon_threshold_mins)?;
    Ok((marked, released))
}

/// List stealable work items.
pub fn list_stealable(db: &dyn WorkStealing, limit: usize) -> Result<Vec<WorkItem>> {
    db.get_stealable_items(limit)
}
