use chrono::Utc;
use flowforge_core::hook::SessionEndInput;
use flowforge_core::Result;

pub fn run() -> Result<()> {
    let ctx = super::HookContext::init()?;
    let _input = SessionEndInput::from_value(&ctx.raw)?;

    if ctx.db.is_none() {
        return Ok(());
    }

    // Capture session data BEFORE ending it (get_current_session filters by ended_at IS NULL)
    let current_session = ctx.with_db("get_current_session", |db| db.get_current_session());
    let current_session = current_session.flatten();

    // Ingest transcript before ending session (only if file exists)
    if let Some(ref session) = current_session {
        let transcript = session
            .transcript_path
            .as_deref()
            .or(_input.common.transcript_path.as_deref());
        if let Some(path) = transcript {
            if std::path::Path::new(path).exists() {
                let sid = session.id.clone();
                let path = path.to_string();
                ctx.with_db("ingest_transcript", |db| db.ingest_transcript(&sid, &path));
            }
        }
    }

    // End current session
    if let Some(ref session) = current_session {
        let sid = session.id.clone();
        ctx.with_db("end_session", |db| db.end_session(&sid, Utc::now()));
    }

    // Log session end to work events (C4)
    if ctx.config.work_tracking.log_all {
        if let Some(ref session) = current_session {
            if let Some(wid) = ctx.resolve_work_item_for_task(None) {
                ctx.record_work_event(
                    &wid,
                    "session_ended",
                    None,
                    Some(&format!(
                        "edits: {}, commands: {}",
                        session.edits, session.commands
                    )),
                    Some("hook:session-end"),
                );
            }
        }
    }

    // Push FlowForge-only items to external backend (C4)
    ctx.with_db("push_to_backend", |db| {
        flowforge_core::work_tracking::push_to_backend(db, &ctx.config.work_tracking)
    });

    // Close active trajectory, judge it, distill if successful
    if let Some(ref session) = current_session {
        let sid = session.id.clone();
        ctx.with_db("trajectory_judgment", |db| {
            let trajectory = match db.get_active_trajectory(&sid)? {
                Some(t) => t,
                None => return Ok(()),
            };

            use flowforge_core::trajectory::TrajectoryStatus;
            db.end_trajectory(&trajectory.id, TrajectoryStatus::Completed)?;

            // Judge and distill, then feed verdict back to routing weights
            use flowforge_memory::trajectory::TrajectoryJudge;
            let judge = TrajectoryJudge::new(db, &ctx.config.patterns);
            let result = match judge.judge(&trajectory.id) {
                Ok(r) => r,
                Err(_) => return Ok(()),
            };

            if result.verdict == flowforge_core::trajectory::TrajectoryVerdict::Success {
                let _ = judge.distill(&trajectory.id);
            }

            // Effectiveness feedback: routing accuracy + pattern confidence boost
            if let Ok(injections) = db.get_injections_for_session(&sid) {
                // Routing accuracy: compare suggested agent vs actual
                for inj in injections.iter().filter(|i| i.injection_type == "routing") {
                    let hit = trajectory
                        .agent_name
                        .as_ref()
                        .map(|a| a.eq_ignore_ascii_case(&inj.reference_id))
                        .unwrap_or(false);
                    let _ =
                        db.set_meta(&format!("routing_hit:{}", sid), if hit { "1" } else { "0" });
                }

                // Pattern confidence feedback based on verdict
                {
                    use flowforge_core::trajectory::TrajectoryVerdict;
                    let store = flowforge_memory::PatternStore::new(db, &ctx.config.patterns);
                    match result.verdict {
                        TrajectoryVerdict::Success => {
                            for inj in injections.iter().filter(|i| i.injection_type == "pattern") {
                                let _ = store.record_feedback(&inj.reference_id, true);
                            }
                        }
                        TrajectoryVerdict::Failure => {
                            for inj in injections.iter().filter(|i| i.injection_type == "pattern") {
                                let _ = store.record_feedback(&inj.reference_id, false);
                            }
                        }
                        TrajectoryVerdict::Partial => {} // avoid noise
                    }
                }
            }

            // Auto-rate all context injections based on trajectory verdict
            {
                use flowforge_core::trajectory::TrajectoryVerdict;
                let rating = match result.verdict {
                    TrajectoryVerdict::Success => "correlated_success",
                    TrajectoryVerdict::Failure => "correlated_failure",
                    TrajectoryVerdict::Partial => "correlated_partial",
                };
                let _ = db.rate_session_injections(&sid, rating);
            }

            // Feed verdict back to routing weights (close the learning loop)
            if let (Some(ref agent_name), Some(ref task_desc)) =
                (&trajectory.agent_name, &trajectory.task_description)
            {
                let pattern = crate::hooks::extract_task_pattern(task_desc);
                if !pattern.is_empty() {
                    use flowforge_core::trajectory::TrajectoryVerdict;
                    match result.verdict {
                        TrajectoryVerdict::Success => {
                            let _ = db.record_routing_success(&pattern, agent_name);
                        }
                        TrajectoryVerdict::Failure => {
                            let _ = db.record_routing_failure(&pattern, agent_name);
                        }
                        TrajectoryVerdict::Partial => {} // avoid noise
                    }
                    // Store routing embedding for similarity-based generalization
                    let config_for_embed = flowforge_core::config::PatternsConfig::default();
                    let embedding = flowforge_memory::default_embedder(&config_for_embed);
                    let vec = embedding.embed(&pattern);
                    let source_id = format!("{}::{}", pattern, agent_name);
                    let _ = db.store_vector("routing", &source_id, &vec);
                }
            }

            // Consolidate old trajectories
            let _ = judge.consolidate();

            Ok(())
        });
    }

    // Run pattern consolidation (wrapped in transaction for atomicity)
    if ctx.config.hooks.learning {
        ctx.with_db("pattern_consolidation", |db| {
            db.with_transaction(|| {
                let store = flowforge_memory::PatternStore::new(db, &ctx.config.patterns);
                store.consolidate()
            })
        });
    }

    Ok(())
}
