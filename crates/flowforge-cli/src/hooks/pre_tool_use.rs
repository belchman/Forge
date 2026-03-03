use flowforge_core::hook::{self, PreToolUseInput, PreToolUseOutput};
use flowforge_core::Result;

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
    }

    // Allow the tool use (exit 0, no output needed for allow)
    Ok(())
}
