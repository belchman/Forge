use flowforge_core::hook::{self, PostToolUseFailureInput};
use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;

pub fn run() -> Result<()> {
    let input: PostToolUseFailureInput = hook::parse_stdin()?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    // Record failed tool use for error pattern tracking
    if config.hooks.learning {
        let db_path = config.db_path();
        if db_path.exists() {
            if let Ok(db) = MemoryDb::open(&db_path) {
                let error_msg = input.error.as_deref().unwrap_or("unknown error");
                let pattern = format!(
                    "tool_failure:{} - {}",
                    input.tool_name,
                    &error_msg[..error_msg.len().min(100)]
                );
                let store = flowforge_memory::PatternStore::new(&db, &config.patterns);
                let _ = store.store_short_term(&pattern, "error_pattern");
            }
        }
    }

    Ok(())
}
