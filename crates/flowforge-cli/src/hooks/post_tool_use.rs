use flowforge_core::hook::{self, PostToolUseInput};
use flowforge_core::{EditRecord, FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;
use chrono::Utc;
use std::path::Path;

pub fn run() -> Result<()> {
    let input: PostToolUseInput = hook::parse_stdin()?;

    // Record edits for Write, Edit, MultiEdit operations
    match input.tool_name.as_str() {
        "Write" | "Edit" | "MultiEdit" => {
            record_edit(&input)?;
        }
        _ => {}
    }

    Ok(())
}

fn record_edit(input: &PostToolUseInput) -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;

    if !config.hooks.edit_tracking {
        return Ok(());
    }

    let db_path = config.db_path();
    if !db_path.exists() {
        return Ok(());
    }

    let db = MemoryDb::open(&db_path)?;

    let file_path = input
        .tool_input
        .get("file_path")
        .or_else(|| input.tool_input.get("filePath"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let extension = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());

    let session_id = db
        .get_current_session()?
        .map(|s| s.id)
        .unwrap_or_else(|| "unknown".to_string());

    let edit = EditRecord {
        session_id: session_id.clone(),
        timestamp: Utc::now(),
        file_path: file_path.to_string(),
        operation: input.tool_name.clone(),
        file_extension: extension,
    };

    db.record_edit(&edit)?;
    db.increment_session_edits(&session_id)?;

    Ok(())
}
