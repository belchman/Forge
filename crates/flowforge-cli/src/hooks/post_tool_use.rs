use chrono::Utc;
use flowforge_core::hook::PostToolUseInput;
use flowforge_core::{EditRecord, Result};
use flowforge_memory::MemoryDb;
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn run() -> Result<()> {
    let ctx = super::HookContext::init()?;
    let input = PostToolUseInput::from_value(&ctx.raw)?;

    if ctx.db.is_none() {
        return Ok(());
    }

    // Record edits for Write, Edit, MultiEdit operations
    if ctx.config.hooks.edit_tracking {
        match input.tool_name.as_str() {
            "Write" | "Edit" | "MultiEdit" => {
                ctx.with_db("record_edit", |db| record_edit(&input, db));
            }
            _ => {}
        }
    }

    // Record trajectory step
    ctx.with_db("record_trajectory_step", |db| {
        if let Some(session) = db.get_current_session()? {
            if let Some(trajectory) = db.get_active_trajectory(&session.id)? {
                // Hash tool_input for privacy
                let input_str = serde_json::to_string(&input.tool_input).unwrap_or_default();
                let input_hash = format!("{:x}", Sha256::digest(input_str.as_bytes()));
                db.record_trajectory_step(
                    &trajectory.id,
                    &input.tool_name,
                    Some(&input_hash),
                    flowforge_core::trajectory::StepOutcome::Success,
                    None,
                )?;
            }
        }
        Ok(())
    });

    Ok(())
}

fn record_edit(input: &PostToolUseInput, db: &MemoryDb) -> Result<()> {
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
