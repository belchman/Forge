//! Hook I/O types for Claude Code integration.
//! Each hook receives JSON on stdin and may return JSON on stdout.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Common Hook Fields (B5) ──
// All hook inputs can optionally contain these common fields.

/// Common fields present in all hook inputs from Claude Code.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CommonHookFields {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

// ── Hook Input Types ──

#[derive(Debug, Clone, Deserialize)]
pub struct PreToolUseInput {
    pub tool_name: String,
    pub tool_input: Value,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostToolUseInput {
    pub tool_name: String,
    pub tool_input: Value,
    #[serde(default)]
    pub tool_response: Option<Value>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

/// Input for PostToolUseFailure events (B2).
#[derive(Debug, Clone, Deserialize)]
pub struct PostToolUseFailureInput {
    pub tool_name: String,
    pub tool_input: Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

/// Input for Notification events (B2).
#[derive(Debug, Clone, Deserialize)]
pub struct NotificationInput {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserPromptSubmitInput {
    pub prompt: String,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionStartInput {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionEndInput {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StopInput {
    #[serde(default)]
    pub stop_hook_active: bool,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreCompactInput {
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentStartInput {
    pub agent_id: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentStopInput {
    pub agent_id: String,
    #[serde(default)]
    pub last_assistant_message: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TeammateIdleInput {
    pub teammate_name: String,
    #[serde(default)]
    pub team_name: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskCompletedInput {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub task_subject: Option<String>,
    #[serde(default)]
    pub teammate_name: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookFields,
}

// ── Hook Output Types ──

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PermissionInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_input: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_context: Option<String>,
}

/// Output for PreToolUse hooks with permission decisions (B4).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreToolUseOutput {
    hook_specific_output: PermissionInner,
}

impl PreToolUseOutput {
    /// Allow the tool use (skip output, no prompt).
    pub fn allow() -> Self {
        Self {
            hook_specific_output: PermissionInner {
                permission_decision: None,
                reason: None,
                updated_input: None,
            },
        }
    }

    /// Explicitly allow and skip user confirmation.
    pub fn allow_explicit() -> Self {
        Self {
            hook_specific_output: PermissionInner {
                permission_decision: Some("allow".to_string()),
                reason: None,
                updated_input: None,
            },
        }
    }

    /// Deny the tool use with a reason.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            hook_specific_output: PermissionInner {
                permission_decision: Some("deny".to_string()),
                reason: Some(reason.into()),
                updated_input: None,
            },
        }
    }

    /// Force user confirmation before proceeding.
    pub fn ask(reason: impl Into<String>) -> Self {
        Self {
            hook_specific_output: PermissionInner {
                permission_decision: Some("ask".to_string()),
                reason: Some(reason.into()),
                updated_input: None,
            },
        }
    }

    /// Allow but modify the tool input before execution (e.g., add --dry-run).
    pub fn allow_with_updated_input(updated_input: Value) -> Self {
        Self {
            hook_specific_output: PermissionInner {
                permission_decision: None,
                reason: None,
                updated_input: Some(updated_input),
            },
        }
    }
}

/// Output for hooks that provide additional context.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextOutput {
    hook_specific_output: ContextInner,
}

impl ContextOutput {
    pub fn none() -> Self {
        Self {
            hook_specific_output: ContextInner {
                additional_context: None,
            },
        }
    }

    pub fn with_context(context: impl Into<String>) -> Self {
        Self {
            hook_specific_output: ContextInner {
                additional_context: Some(context.into()),
            },
        }
    }
}

// ── Dangerous Command Patterns ──

/// Patterns that should be blocked or warned about in Bash commands
pub const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf /", "Recursive delete of root filesystem"),
    ("rm -rf ~", "Recursive delete of home directory"),
    ("rm -rf /*", "Recursive delete of all root contents"),
    (":(){:|:&};:", "Fork bomb"),
    ("mkfs.", "Filesystem formatting"),
    ("dd if=/dev/zero", "Disk overwrite with zeros"),
    ("dd if=/dev/random", "Disk overwrite with random data"),
    ("> /dev/sda", "Direct disk overwrite"),
    ("chmod -R 777 /", "Remove all permissions from root"),
    ("wget|sh", "Pipe download to shell"),
    ("curl|sh", "Pipe download to shell"),
    ("curl|bash", "Pipe download to bash"),
    ("wget|bash", "Pipe download to bash"),
    ("--no-preserve-root", "Bypasses root protection"),
    ("sudo rm -rf", "Sudo recursive force delete"),
];

/// Check if a bash command matches any dangerous pattern
pub fn check_dangerous_command(command: &str) -> Option<&'static str> {
    let cmd_lower = command.to_lowercase();
    let cmd_normalized = cmd_lower.replace('\\', "").replace('\n', " ");

    for (pattern, reason) in DANGEROUS_PATTERNS {
        if cmd_normalized.contains(&pattern.to_lowercase()) {
            return Some(reason);
        }
    }
    None
}

/// Read hook input from stdin
pub fn read_stdin() -> crate::Result<String> {
    use std::io::Read;
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    Ok(input)
}

/// Parse hook input from stdin as JSON
pub fn parse_stdin<T: serde::de::DeserializeOwned>() -> crate::Result<T> {
    let input = read_stdin()?;
    if input.trim().is_empty() {
        return Err(crate::Error::Hook("Empty stdin input".to_string()));
    }
    serde_json::from_str(&input).map_err(|e| crate::Error::Hook(format!("Invalid JSON input: {e}")))
}

/// Write hook output as JSON to stdout
pub fn write_stdout<T: Serialize>(output: &T) -> crate::Result<()> {
    use std::io::Write;
    let json = serde_json::to_string(output)?;
    println!("{json}");
    std::io::stdout().flush()?;
    Ok(())
}
