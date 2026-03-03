use flowforge_core::hook::{self, PreToolUseInput, PreToolUseOutput};
use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;

pub fn run() -> Result<()> {
    let input: PreToolUseInput = hook::parse_stdin()?;

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

        // Track command count for non-blocked Bash commands
        let _ = increment_command_count();
    }

    // Allow the tool use (exit 0, no output needed for allow)
    Ok(())
}

fn increment_command_count() -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let db_path = config.db_path();
    if !db_path.exists() {
        return Ok(());
    }
    let db = MemoryDb::open(&db_path)?;
    if let Some(session) = db.get_current_session()? {
        db.increment_session_commands(&session.id)?;
    }
    Ok(())
}
