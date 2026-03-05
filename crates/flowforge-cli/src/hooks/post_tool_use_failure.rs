use flowforge_core::hook::PostToolUseFailureInput;
use flowforge_core::Result;
use sha2::{Digest, Sha256};

pub fn run() -> Result<()> {
    let ctx = super::HookContext::init()?;
    let input = PostToolUseFailureInput::from_value(&ctx.raw)?;

    // Record failed tool use for error pattern tracking
    if ctx.config.hooks.learning {
        ctx.with_db("store_error_pattern", |db| {
            let error_msg = input.error.as_deref().unwrap_or("unknown error");
            let truncated: String = error_msg.chars().take(100).collect();
            let pattern = format!("tool_failure:{} - {}", input.tool_name, truncated);
            let store = flowforge_memory::PatternStore::new(db, &ctx.config.patterns);
            store.store_short_term(&pattern, "error_pattern")
        });
    }

    // Record trajectory failure step
    ctx.with_db("record_trajectory_failure_step", |db| {
        if let Some(session) = db.get_current_session()? {
            if let Some(trajectory) = db.get_active_trajectory(&session.id)? {
                let input_str = serde_json::to_string(&input.tool_input).unwrap_or_default();
                let input_hash = format!("{:x}", Sha256::digest(input_str.as_bytes()));
                db.record_trajectory_step(
                    &trajectory.id,
                    &input.tool_name,
                    Some(&input_hash),
                    flowforge_core::trajectory::StepOutcome::Failure,
                    None,
                )?;
            }
        }
        Ok(())
    });

    Ok(())
}
