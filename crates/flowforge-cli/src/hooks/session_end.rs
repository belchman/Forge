use flowforge_core::hook::{self, SessionEndInput};
use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;
use chrono::Utc;

pub fn run() -> Result<()> {
    let _input: SessionEndInput = hook::parse_stdin()?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    let db_path = config.db_path();
    if !db_path.exists() {
        return Ok(());
    }

    let db = MemoryDb::open(&db_path)?;

    // End current session
    if let Ok(Some(session)) = db.get_current_session() {
        db.end_session(&session.id, Utc::now())?;
    }

    // Run pattern consolidation
    if config.hooks.learning {
        let store = flowforge_memory::PatternStore::new(&db, &config.patterns);
        let _ = store.consolidate();
    }

    Ok(())
}
