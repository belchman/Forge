//! Plugin command execution: spawn plugin tools and hooks as child processes.

use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::Result;

/// Execute a plugin tool command.
/// Sends params as JSON to stdin, reads JSON from stdout.
pub fn exec_plugin_tool(
    command: &str,
    dir: &Path,
    params: &Value,
    timeout_ms: u64,
) -> Result<Value> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(crate::Error::Plugin("Empty command".to_string()));
    }

    let mut cmd = Command::new(parts[0]);
    for arg in &parts[1..] {
        cmd.arg(arg);
    }

    cmd.current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| crate::Error::Plugin(format!("Failed to spawn plugin command: {e}")))?;

    // Write params to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let json = serde_json::to_string(params)
            .map_err(|e| crate::Error::Plugin(format!("JSON serialize error: {e}")))?;
        let _ = stdin.write_all(json.as_bytes());
    }

    // Enforce timeout: spawn a killer thread that terminates the child if it exceeds the deadline
    let timeout = Duration::from_millis(timeout_ms);
    let child_id = child.id();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();

    std::thread::spawn(move || {
        if done_rx.recv_timeout(timeout).is_err() {
            // Timeout expired — kill the child process
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(child_id as i32, libc::SIGKILL);
                }
            }
            #[cfg(not(unix))]
            {
                // Best-effort on non-Unix; wait_with_output will still return
                let _ = child_id;
            }
        }
    });

    let output = match child.wait_with_output() {
        Ok(o) => {
            let _ = done_tx.send(()); // Cancel the killer thread
            o
        }
        Err(e) => {
            let _ = done_tx.send(());
            return Err(crate::Error::Plugin(format!("Plugin command failed: {e}")));
        }
    };

    // Detect if killed by timeout (signal 9 on Unix)
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if output.status.signal() == Some(9) {
            return Err(crate::Error::Plugin(format!(
                "Plugin command timed out after {timeout_ms}ms"
            )));
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::Plugin(format!(
            "Plugin command exited with {}: {}",
            output.status,
            stderr.chars().take(500).collect::<String>()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| crate::Error::Plugin(format!("Invalid JSON from plugin: {e}")))
}

/// Execute a plugin hook command.
/// Returns Some(value) if the hook produced output, None otherwise.
pub fn exec_plugin_hook(
    command: &str,
    dir: &Path,
    input: &Value,
    timeout_ms: u64,
) -> Option<Value> {
    match exec_plugin_tool(command, dir, input, timeout_ms) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Plugin hook failed: {e}");
            None
        }
    }
}
