use flowforge_core::hook::{self, ContextOutput, SessionStartInput};
use flowforge_core::{FlowForgeConfig, Result, SessionInfo};
use flowforge_memory::MemoryDb;
use chrono::Utc;
use uuid::Uuid;

pub fn run() -> Result<()> {
    let input: SessionStartInput = hook::parse_stdin()?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    let db_path = config.db_path();
    if !db_path.exists() {
        // Not initialized, just return context
        let output = ContextOutput::with_context(
            "[FlowForge] Not initialized. Run `flowforge init --project` to set up.".to_string(),
        );
        hook::write_stdout(&output)?;
        return Ok(());
    }

    let db = MemoryDb::open(&db_path)?;

    // Create new session
    let session_id = input
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let session = SessionInfo {
        id: session_id.clone(),
        started_at: Utc::now(),
        ended_at: None,
        cwd,
        edits: 0,
        commands: 0,
        summary: None,
    };

    db.create_session(&session)?;

    // Build context with session info and stats
    let mut context_parts = vec![
        format!("[FlowForge] Session {} started.", &session_id[..8]),
    ];

    // Include stats from previous sessions
    if let Ok(sessions) = db.list_sessions(5) {
        if sessions.len() > 1 {
            let total_edits: u64 = sessions.iter().map(|s| s.edits).sum();
            context_parts.push(format!(
                "Recent activity: {} sessions, {} total edits.",
                sessions.len(),
                total_edits
            ));
        }
    }

    // Include pattern count
    if let Ok(count) = db.count_patterns() {
        if count > 0 {
            context_parts.push(format!("{count} learned patterns available."));
        }
    }

    let output = ContextOutput::with_context(context_parts.join(" "));
    hook::write_stdout(&output)?;

    Ok(())
}
