use flowforge_core::hook::{self, NotificationInput};
use flowforge_core::{FlowForgeConfig, Result};

pub fn run() -> Result<()> {
    let input: NotificationInput = hook::parse_stdin()?;

    // Log notifications to the hook error log for audit trail
    if let Some(message) = &input.message {
        let log_path = FlowForgeConfig::project_dir().join("hook-errors.log");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            use std::io::Write;
            let timestamp = chrono::Utc::now().to_rfc3339();
            let level = input.level.as_deref().unwrap_or("info");
            let _ = writeln!(file, "[{}] notification({}): {}", timestamp, level, message);
        }
    }

    Ok(())
}
