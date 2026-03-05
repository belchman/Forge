//! Individual gate check functions for the guidance engine.

use serde_json::Value;

use crate::config::GuidanceConfig;
use crate::types::{GateAction, RiskLevel, RuleScope};

use super::patterns::{DESTRUCTIVE_PATTERNS, SECRET_PATTERNS};
use super::CompiledRule;

/// Check destructive operations gate.
pub(super) fn check_destructive(
    tool_name: &str,
    tool_input: &Value,
) -> Option<(GateAction, String)> {
    // Check bash commands
    if tool_name == "Bash" {
        if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
            let cmd_lower = cmd.to_lowercase();
            for (regex, desc, risk) in DESTRUCTIVE_PATTERNS.iter() {
                if regex.is_match(&cmd_lower) {
                    let action = match risk {
                        RiskLevel::Critical => GateAction::Deny,
                        RiskLevel::High => GateAction::Deny,
                        RiskLevel::Medium => GateAction::Ask,
                        RiskLevel::Low => GateAction::Ask,
                    };
                    return Some((action, format!("[destructive_ops] {desc}")));
                }
            }
        }
    }

    // Check all tools for SQL injection patterns
    let input_str = tool_input.to_string().to_lowercase();
    let sql_patterns = [
        ("drop table", "SQL DROP TABLE detected"),
        ("drop database", "SQL DROP DATABASE detected"),
        ("delete from", "SQL DELETE FROM detected"),
        ("truncate table", "SQL TRUNCATE TABLE detected"),
    ];
    for (pattern, desc) in &sql_patterns {
        if input_str.contains(pattern) {
            return Some((GateAction::Ask, format!("[destructive_ops] {desc}")));
        }
    }

    None
}

/// Check secrets detection gate.
pub(super) fn check_secrets(tool_input: &Value) -> Option<(GateAction, String)> {
    let input_str = tool_input.to_string();
    for regex in SECRET_PATTERNS.iter() {
        if regex.is_match(&input_str) {
            return Some((
                GateAction::Deny,
                "[secrets] Potential secret/credential detected in tool input".to_string(),
            ));
        }
    }
    None
}

/// Check file scope gate.
pub(super) fn check_file_scope(
    tool_name: &str,
    tool_input: &Value,
    protected_paths: &[String],
) -> Option<(GateAction, String)> {
    if !matches!(tool_name, "Write" | "Edit" | "MultiEdit") {
        return None;
    }

    let file_path = tool_input
        .get("file_path")
        .or_else(|| tool_input.get("filePath"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if file_path.is_empty() {
        return None;
    }

    for protected in protected_paths {
        if glob_match(protected, file_path) {
            return Some((
                GateAction::Deny,
                format!("[file_scope] Write to protected path: {file_path} (matches {protected})"),
            ));
        }
    }

    None
}

/// Check custom rules gate.
pub(super) fn check_custom_rule(
    rule: &CompiledRule,
    tool_name: &str,
    tool_input: &Value,
) -> Option<(GateAction, String)> {
    let text = match rule.scope {
        RuleScope::Tool => tool_name.to_string(),
        RuleScope::Command => tool_input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        RuleScope::File => tool_input
            .get("file_path")
            .or_else(|| tool_input.get("filePath"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    };

    if rule.regex.is_match(&text) {
        Some((
            rule.action,
            format!("[custom:{}] {}", rule.id, rule.description),
        ))
    } else {
        None
    }
}

/// Check diff size gate.
pub(super) fn check_diff_size(
    tool_name: &str,
    tool_input: &Value,
    config: &GuidanceConfig,
) -> Option<(GateAction, String)> {
    if !matches!(tool_name, "Write" | "Edit" | "MultiEdit") {
        return None;
    }

    // Estimate lines from content/new_string
    let content = tool_input
        .get("content")
        .or_else(|| tool_input.get("new_string"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let lines = content.lines().count();
    if lines > config.max_diff_lines {
        return Some((
            GateAction::Ask,
            format!(
                "[diff_size] Edit changes ~{lines} lines (max: {})",
                config.max_diff_lines
            ),
        ));
    }

    None
}

/// Simple glob matching for protected paths.
pub(super) fn glob_match(pattern: &str, path: &str) -> bool {
    let path_lower = path.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    if let Some(suffix) = pattern_lower.strip_prefix('*') {
        // *.ext or *keyword*
        if let Some(middle) = suffix.strip_suffix('*') {
            return path_lower.contains(middle);
        }
        return path_lower.ends_with(suffix);
    }

    if pattern_lower.ends_with("/*") {
        let prefix = &pattern_lower[..pattern_lower.len() - 2];
        return path_lower.starts_with(prefix) || path_lower.contains(&format!("/{prefix}/"));
    }

    if pattern_lower.contains('*') {
        // .env.* pattern
        let parts: Vec<&str> = pattern_lower.split('*').collect();
        if parts.len() == 2 {
            return path_lower.starts_with(parts[0])
                || std::path::Path::new(path)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map(|f| f.to_lowercase().starts_with(parts[0]))
                    .unwrap_or(false);
        }
    }

    // Exact match or filename match
    path_lower == pattern_lower
        || std::path::Path::new(path)
            .file_name()
            .and_then(|f| f.to_str())
            .map(|f| f.to_lowercase() == pattern_lower)
            .unwrap_or(false)
}
