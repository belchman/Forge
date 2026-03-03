pub mod notification;
pub mod post_tool_use;
pub mod post_tool_use_failure;
pub mod pre_compact;
pub mod pre_tool_use;
pub mod session_end;
pub mod session_start;
pub mod stop;
pub mod subagent_start;
pub mod subagent_stop;
pub mod task_completed;
pub mod teammate_idle;
pub mod user_prompt_submit;

use std::io::Write;

/// Log a hook error to .flowforge/hook-errors.log instead of crashing.
fn log_hook_error(hook_name: &str, error: &dyn std::fmt::Display) {
    let log_path = flowforge_core::FlowForgeConfig::project_dir().join("hook-errors.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let _ = writeln!(file, "[{}] {}: {}", timestamp, hook_name, error);
    }
    tracing::error!("Hook {} failed: {}", hook_name, error);
}

/// Run a hook safely: catch errors, log them, and return Ok(()) regardless.
pub fn run_safe(
    hook_name: &str,
    f: impl FnOnce() -> flowforge_core::Result<()>,
) -> flowforge_core::Result<()> {
    match f() {
        Ok(()) => Ok(()),
        Err(e) => {
            log_hook_error(hook_name, &e);
            Ok(())
        }
    }
}
