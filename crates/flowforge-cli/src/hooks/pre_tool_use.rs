use flowforge_core::hook::{self, PreToolUseInput, PreToolUseOutput};
use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;
use serde_json::json;
use sha2::{Digest, Sha256};

pub fn run() -> Result<()> {
    let v = hook::parse_stdin_value()?;
    let input = PreToolUseInput::from_value(&v)?;
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let db_path = config.db_path();

    if !db_path.exists() {
        // Fall back to just the dangerous command check without DB
        if input.tool_name == "Bash" {
            if let Some(command) = input.tool_input.get("command").and_then(|v| v.as_str()) {
                if let Some(reason) = hook::check_dangerous_command(command) {
                    let output = PreToolUseOutput::deny(format!(
                        "FlowForge blocked dangerous command: {reason}"
                    ));
                    hook::write_stdout(&output)?;
                    return Ok(());
                }
            }
        }
        return Ok(());
    }

    let db = MemoryDb::open(&db_path)?;

    // Resolve session_id once and reuse everywhere
    let session_id = db
        .get_current_session()
        .ok()
        .flatten()
        .map(|s| s.id)
        .unwrap_or_else(|| "unknown".to_string());

    // 1. Guidance gates (if enabled)
    if config.guidance.enabled {
        let engine = match flowforge_core::guidance::GuidanceEngine::from_config(&config.guidance) {
            Ok(e) => e,
            Err(_) => {
                // If guidance engine fails to initialize, skip guidance gates
                return Ok(());
            }
        };

        // Get or create trust score for current session
        let session_id = session_id.clone();

        let trust = db
            .get_trust_score(&session_id)
            .ok()
            .flatten()
            .map(|t| t.score)
            .unwrap_or(config.guidance.trust_initial_score);

        let (action, reason, rule_id) = engine.evaluate(&input.tool_name, &input.tool_input, trust);

        // Calculate trust delta based on action
        let trust_delta = match action {
            flowforge_core::types::GateAction::Deny => -0.1,
            flowforge_core::types::GateAction::Ask => -0.02,
            flowforge_core::types::GateAction::Allow => 0.01,
        };

        // Update trust score
        let _ = db.update_trust_score(&session_id, &action, trust_delta);

        // Record non-allow decisions in audit log
        if action != flowforge_core::types::GateAction::Allow {
            let risk_level = if rule_id.is_some() {
                flowforge_core::types::RiskLevel::Medium
            } else {
                flowforge_core::types::RiskLevel::High
            };

            // Get previous hash for chain
            let prev_hash = db
                .get_gate_decisions(&session_id, 1)
                .ok()
                .and_then(|decisions| decisions.into_iter().next())
                .map(|d| d.hash)
                .unwrap_or_default();

            let new_trust = (trust + trust_delta).clamp(0.0, 1.0);
            let hash_input = format!("{}{}{}{}", session_id, input.tool_name, reason, prev_hash);
            let hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));

            let decision = flowforge_core::types::GateDecision {
                id: 0,
                session_id: session_id.clone(),
                rule_id: rule_id.clone(),
                gate_name: "guidance".to_string(),
                tool_name: input.tool_name.clone(),
                action,
                reason: reason.clone(),
                risk_level,
                trust_before: trust,
                trust_after: new_trust,
                timestamp: chrono::Utc::now(),
                hash,
                prev_hash,
            };
            let _ = db.record_gate_decision(&decision);
        }

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
    if let Ok(plugins) = flowforge_core::plugin::load_all_plugins(&config.plugins) {
        if !plugins.is_empty() {
            let raw_input = json!({
                "tool_name": input.tool_name,
                "tool_input": input.tool_input,
            });
            let plugins_dir = FlowForgeConfig::plugins_dir();
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

    // 3. Work-stealing heartbeat (piggyback on every tool use)
    if config.work_tracking.work_stealing.enabled {
        let _ = db.update_heartbeat(&session_id);
    }

    // 4. Work-item enforcement gate: block mutating tools when no active work item
    //    Toggle off with FLOWFORGE_NO_WORK_GATE=1 or config work_tracking.enforce_gate = false
    if config.work_tracking.require_task
        && config.work_tracking.enforce_gate
        && std::env::var("FLOWFORGE_NO_WORK_GATE").is_err()
    {
        let is_safe = config
            .guidance
            .safe_tools
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&input.tool_name));

        if !is_safe {
            // Allow flowforge/cargo bash commands through (work management + builds)
            let is_allowed_cmd = input.tool_name == "Bash"
                && input
                    .tool_input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(|cmd| {
                        cmd.contains("flowforge work")
                            || cmd.contains("flowforge init")
                            || cmd.starts_with("cargo ")
                            || cmd.starts_with("git ")
                            || cmd.starts_with("ls")
                            || cmd.starts_with("cat ")
                            || cmd.starts_with("echo ")
                    })
                    .unwrap_or(false);

            // Allow MCP work tools
            let is_work_mcp = input.tool_name.contains("work_create")
                || input.tool_name.contains("work_update")
                || input.tool_name.contains("work_close");

            // Allow coordination tools (team comms, planning, questions)
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

            if !is_allowed_cmd && !is_work_mcp && !is_coordination {
                let filter = flowforge_core::WorkFilter {
                    status: Some("in_progress".to_string()),
                    ..Default::default()
                };
                let has_active = db
                    .list_work_items(&filter)
                    .map(|items| !items.is_empty())
                    .unwrap_or(false);

                if !has_active {
                    let output = PreToolUseOutput::deny(
                        "[FlowForge] BLOCKED: No active kanbus work item. Run `flowforge work create \"<description>\" --type task` first.".to_string(),
                    );
                    hook::write_stdout(&output)?;
                    return Ok(());
                }
            }
        }
    }

    // 5. Existing: dangerous command check for Bash
    if input.tool_name == "Bash" {
        if let Some(command) = input.tool_input.get("command").and_then(|v| v.as_str()) {
            if let Some(reason) = hook::check_dangerous_command(command) {
                let output = PreToolUseOutput::deny(format!(
                    "FlowForge blocked dangerous command: {reason}"
                ));
                hook::write_stdout(&output)?;
                return Ok(());
            }
        }
    }

    // 6. Existing: increment command count
    let _ = db.increment_session_commands(&session_id);

    Ok(())
}
