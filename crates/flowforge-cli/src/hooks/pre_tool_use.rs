use flowforge_core::hook::{self, PreToolUseInput, PreToolUseOutput};
use flowforge_core::Result;
use serde_json::json;
use sha2::{Digest, Sha256};

/// Pre-fetched state from a single DB call, avoiding 5+ round-trips per tool use.
struct PreToolUseState {
    session_id: String,
    trust: f64,
    prev_hash: String,
    needs_heartbeat: bool,
    has_active_work: bool,
}

pub fn run() -> Result<()> {
    let ctx = super::HookContext::init()?;
    let input = PreToolUseInput::from_value(&ctx.raw)?;

    if ctx.db.is_none() {
        // No DB: skip guidance/work-gate but still run bash check + exit
        if check_dangerous_bash(&input)? {
            return Ok(());
        }
        return Ok(());
    }

    // Batch all read queries into a single DB call
    let initial_trust = ctx.config.guidance.trust_initial_score;
    let heartbeat_enabled = ctx.config.work_tracking.work_stealing.enabled;
    let state = ctx
        .with_db("pre_tool_use_state", |db| {
            let session_id = db
                .get_current_session()?
                .map(|s| s.id)
                .unwrap_or_else(|| "unknown".to_string());

            let trust = db
                .get_trust_score(&session_id)
                .ok()
                .flatten()
                .map(|t| t.score)
                .unwrap_or(initial_trust);

            let prev_hash = db
                .get_gate_decisions(&session_id, 1)
                .ok()
                .and_then(|decisions| decisions.into_iter().next())
                .map(|d| d.hash)
                .unwrap_or_default();

            let needs_heartbeat = if heartbeat_enabled {
                match db.get_last_heartbeat_time(&session_id)? {
                    Some(last) => {
                        let elapsed = chrono::Utc::now().signed_duration_since(last);
                        elapsed.num_seconds() >= 30
                    }
                    None => true,
                }
            } else {
                false
            };

            let has_active_work = db
                .list_work_items(&flowforge_core::WorkFilter {
                    status: Some("in_progress".to_string()),
                    ..Default::default()
                })
                .map(|items| !items.is_empty())
                .unwrap_or(false);

            Ok(PreToolUseState {
                session_id,
                trust,
                prev_hash,
                needs_heartbeat,
                has_active_work,
            })
        })
        .unwrap_or(PreToolUseState {
            session_id: "unknown".to_string(),
            trust: initial_trust,
            prev_hash: String::new(),
            needs_heartbeat: false,
            has_active_work: false,
        });

    // 1. Guidance gates (if enabled)
    if ctx.config.guidance.enabled {
        let engine =
            match flowforge_core::guidance::GuidanceEngine::from_config(&ctx.config.guidance) {
                Ok(e) => e,
                Err(e) => {
                    // Guidance init failed — log and skip guidance gates only.
                    // All other checks (heartbeat, work-gate, bash, increment) still run.
                    eprintln!("[FlowForge] guidance init error (skipping gates): {e}");
                    return run_always_checks(&ctx, &input, &state);
                }
            };

        let (action, reason, rule_id) =
            engine.evaluate(&input.tool_name, &input.tool_input, state.trust);

        // Calculate trust delta based on action
        let trust_delta = match action {
            flowforge_core::types::GateAction::Deny => -0.1,
            flowforge_core::types::GateAction::Ask => -0.02,
            flowforge_core::types::GateAction::Allow => 0.01,
        };

        // Update trust score + record gate decision atomically
        let sid = state.session_id.clone();
        ctx.with_db("update_trust_and_gate", |db| {
            db.update_trust_score(&sid, &action, trust_delta)?;

            if action != flowforge_core::types::GateAction::Allow {
                let risk_level = if rule_id.is_some() {
                    flowforge_core::types::RiskLevel::Medium
                } else {
                    flowforge_core::types::RiskLevel::High
                };

                let new_trust = (state.trust + trust_delta).clamp(0.0, 1.0);
                let hash_input = format!(
                    "{}{}{}{}",
                    state.session_id, input.tool_name, reason, state.prev_hash
                );
                let hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));

                let decision = flowforge_core::types::GateDecision {
                    id: 0,
                    session_id: state.session_id.clone(),
                    rule_id: rule_id.clone(),
                    gate_name: "guidance".to_string(),
                    tool_name: input.tool_name.clone(),
                    action,
                    reason: reason.clone(),
                    risk_level,
                    trust_before: state.trust,
                    trust_after: new_trust,
                    timestamp: chrono::Utc::now(),
                    hash,
                    prev_hash: state.prev_hash.clone(),
                };
                db.record_gate_decision(&decision)?;
            }
            Ok(())
        });

        match action {
            flowforge_core::types::GateAction::Deny => {
                let output = PreToolUseOutput::deny(reason);
                hook::write_stdout(&output)?;
                return Ok(());
            }
            flowforge_core::types::GateAction::Ask => {
                let output = PreToolUseOutput::deny(format!("Guidance ask: {reason}"));
                hook::write_stdout(&output)?;
                return Ok(());
            }
            flowforge_core::types::GateAction::Allow => {} // fall through
        }
    }

    // 2. Plugin PreToolUse hooks
    if let Ok(plugins) = flowforge_core::plugin::load_all_plugins(&ctx.config.plugins) {
        if !plugins.is_empty() {
            let raw_input = json!({
                "tool_name": input.tool_name,
                "tool_input": input.tool_input,
            });
            let plugins_dir = flowforge_core::FlowForgeConfig::plugins_dir();
            if let Some(response) =
                super::run_plugin_hooks("PreToolUse", &raw_input, &plugins, &plugins_dir)
            {
                // Plugin returned a deny/ask response
                if let Some(reason) = response.get("reason").and_then(|v| v.as_str()) {
                    let output = PreToolUseOutput::deny(reason.to_string());
                    hook::write_stdout(&output)?;
                    return Ok(());
                }
            }
        }
    }

    // Remaining checks: heartbeat, work-gate, bash validation, command increment
    run_always_checks(&ctx, &input, &state)
}

/// Check bash commands for dangerous patterns and deny if matched.
/// Returns `true` if a dangerous command was blocked (deny written to stdout).
fn check_dangerous_bash(input: &PreToolUseInput) -> Result<bool> {
    if input.tool_name == "Bash" {
        if let Some(command) = input.tool_input.get("command").and_then(|v| v.as_str()) {
            if let Some(reason) = hook::check_dangerous_command(command) {
                let output = PreToolUseOutput::deny(format!(
                    "FlowForge blocked dangerous command: {reason}"
                ));
                hook::write_stdout(&output)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Run checks that must always execute regardless of guidance/plugin status:
/// heartbeat, work-gate enforcement, dangerous command validation, and command increment.
fn run_always_checks(
    ctx: &super::HookContext,
    input: &PreToolUseInput,
    state: &PreToolUseState,
) -> Result<()> {
    // 1. Work-stealing heartbeat (decision already made in batched state)
    if state.needs_heartbeat {
        let sid = state.session_id.clone();
        ctx.with_db("update_heartbeat", |db| db.update_heartbeat(&sid));
    }

    // 2. Work-item enforcement gate (uses pre-fetched has_active_work)
    if ctx.config.work_tracking.require_task
        && ctx.config.work_tracking.enforce_gate
        && std::env::var("FLOWFORGE_NO_WORK_GATE").is_err()
    {
        let is_safe = ctx
            .config
            .guidance
            .safe_tools
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&input.tool_name));

        if !is_safe {
            let is_allowed_cmd = input.tool_name == "Bash"
                && input
                    .tool_input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(|cmd| {
                        cmd.starts_with("flowforge work")
                            || cmd.starts_with("flowforge init")
                            || cmd.starts_with("cargo ")
                            || cmd.starts_with("git ")
                            || cmd.starts_with("ls")
                            || cmd.starts_with("cat ")
                    })
                    .unwrap_or(false);

            let is_work_mcp = input.tool_name.contains("work_create")
                || input.tool_name.contains("work_update")
                || input.tool_name.contains("work_close");

            let is_coordination = matches!(
                input.tool_name.as_str(),
                "SendMessage"
                    | "Skill"
                    | "AskUserQuestion"
                    | "EnterPlanMode"
                    | "ExitPlanMode"
                    | "Task"
                    | "TeamCreate"
                    | "TeamDelete"
            );

            if !is_allowed_cmd && !is_work_mcp && !is_coordination && !state.has_active_work {
                let output = PreToolUseOutput::deny(
                    "[FlowForge] BLOCKED: No active kanbus work item. Run `flowforge work create \"<description>\" --type task` first.".to_string(),
                );
                hook::write_stdout(&output)?;
                return Ok(());
            }
        }
    }

    // 3. Dangerous command check for Bash
    if check_dangerous_bash(input)? {
        return Ok(());
    }

    // 4. Increment command count
    let sid = state.session_id.clone();
    ctx.with_db("increment_session_commands", |db| {
        db.increment_session_commands(&sid)
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_echo_not_in_allowed_commands() {
        // echo should NOT be in the allowed list as it can write to files
        let cmd = "echo 'malicious' > /etc/passwd";
        let is_allowed = cmd.starts_with("flowforge work")
            || cmd.starts_with("flowforge init")
            || cmd.starts_with("cargo ")
            || cmd.starts_with("git ")
            || cmd.starts_with("ls")
            || cmd.starts_with("cat ");
        assert!(!is_allowed);
    }

    #[test]
    fn test_flowforge_work_starts_with_not_contains() {
        // "echo flowforge work" should NOT be allowed
        let cmd = "echo flowforge work create test";
        let is_allowed = cmd.starts_with("flowforge work") || cmd.starts_with("flowforge init");
        assert!(!is_allowed);

        // But actual flowforge commands should be allowed
        let cmd = "flowforge work create test";
        let is_allowed = cmd.starts_with("flowforge work") || cmd.starts_with("flowforge init");
        assert!(is_allowed);
    }
}
