use flowforge_core::hook::PreCompactInput;
use flowforge_core::Result;

pub fn run() -> Result<()> {
    let ctx = super::HookContext::init()?;
    let _input = PreCompactInput::from_value(&ctx.raw)?;

    let mut guidance = vec![
        "[FlowForge Compaction Guidance]".to_string(),
        "Key context to preserve:".to_string(),
    ];

    // Run consolidation and gather context from DB
    ctx.with_db("compaction_context", |db| {
        let store = flowforge_memory::PatternStore::new(db, &ctx.config.patterns);
        store.consolidate()?;

        // Include current session stats
        if let Some(session) = db.get_current_session()? {
            guidance.push(format!(
                "- Current session: {} edits, {} commands",
                session.edits, session.commands
            ));
        }

        // Include recent patterns
        if let Ok(patterns) = db.get_top_patterns(5) {
            if !patterns.is_empty() {
                guidance.push("- Active patterns:".to_string());
                for p in &patterns {
                    guidance.push(format!(
                        "  - [{}] {} (conf: {:.0}%)",
                        p.category,
                        p.content,
                        p.confidence * 100.0
                    ));
                }
            }
        }

        Ok(())
    });

    guidance
        .push("- Use `flowforge memory search <query>` to recall stored knowledge.".to_string());
    guidance.push("- Use `flowforge session current` to check session state.".to_string());

    let output = flowforge_core::hook::ContextOutput::with_context(guidance.join("\n"));
    output.write()?;

    Ok(())
}
