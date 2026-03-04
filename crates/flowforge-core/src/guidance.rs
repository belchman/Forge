//! Guidance Control Plane: rule engine for evaluating tool uses.

use regex::Regex;
use serde_json::Value;

use crate::config::GuidanceConfig;
use crate::types::{GateAction, RiskLevel};
use crate::Result;

/// Compiled guidance engine with regex patterns for all gates.
pub struct GuidanceEngine {
    destructive_patterns: Vec<(Regex, &'static str, RiskLevel)>,
    secret_patterns: Vec<Regex>,
    protected_paths: Vec<String>,
    custom_rules: Vec<CompiledRule>,
    config: GuidanceConfig,
}

struct CompiledRule {
    id: String,
    regex: Regex,
    action: GateAction,
    scope: crate::types::RuleScope,
    description: String,
}

impl GuidanceEngine {
    /// Build the engine from config. Compiles all regex patterns.
    pub fn from_config(config: &GuidanceConfig) -> Result<Self> {
        let destructive_patterns = Self::build_destructive_patterns();
        let secret_patterns = Self::build_secret_patterns();

        let mut custom_rules = Vec::new();
        for rule in &config.custom_rules {
            if !rule.enabled {
                continue;
            }
            let regex = Regex::new(&rule.pattern).map_err(|e| {
                crate::Error::Guidance(format!("Invalid rule pattern '{}': {e}", rule.pattern))
            })?;
            custom_rules.push(CompiledRule {
                id: rule.id.clone(),
                regex,
                action: rule.action,
                scope: rule.scope,
                description: rule.description.clone(),
            });
        }

        let mut protected = vec![
            ".env".to_string(),
            ".env.*".to_string(),
            "*.key".to_string(),
            "*.pem".to_string(),
            ".ssh/*".to_string(),
            "*credentials*".to_string(),
            "*secret*".to_string(),
        ];
        protected.extend(config.protected_paths.iter().cloned());

        Ok(Self {
            destructive_patterns,
            secret_patterns,
            protected_paths: protected,
            custom_rules,
            config: config.clone(),
        })
    }

    /// Evaluate a tool use against all gates.
    /// Returns (action, reason, optional rule_id).
    /// First deny wins. If trust >= threshold, ask -> allow.
    pub fn evaluate(
        &self,
        tool_name: &str,
        tool_input: &Value,
        trust: f64,
    ) -> (GateAction, String, Option<String>) {
        // Gate 1: Destructive operations
        if self.config.destructive_ops_gate {
            if let Some((action, reason)) = self.check_destructive(tool_name, tool_input) {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        // Gate 2: Secrets detection
        if self.config.secrets_gate {
            if let Some((action, reason)) = self.check_secrets(tool_input) {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        // Gate 3: File scope
        if self.config.file_scope_gate {
            if let Some((action, reason)) = self.check_file_scope(tool_name, tool_input) {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        // Gate 4: Custom rules
        for rule in &self.custom_rules {
            if let Some((action, reason)) = self.check_custom_rule(rule, tool_name, tool_input) {
                return self.apply_trust(action, reason, Some(rule.id.clone()), trust);
            }
        }

        // Gate 5: Diff size
        if self.config.diff_size_gate {
            if let Some((action, reason)) = self.check_diff_size(tool_name, tool_input) {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        (GateAction::Allow, "all gates passed".to_string(), None)
    }

    fn apply_trust(
        &self,
        action: GateAction,
        reason: String,
        rule_id: Option<String>,
        trust: f64,
    ) -> (GateAction, String, Option<String>) {
        // Trust-based relaxation: if score >= threshold, ask -> allow
        if action == GateAction::Ask && trust >= self.config.trust_ask_threshold {
            return (
                GateAction::Allow,
                format!(
                    "{reason} (auto-approved: trust {trust:.2} >= {:.2})",
                    self.config.trust_ask_threshold
                ),
                rule_id,
            );
        }
        (action, reason, rule_id)
    }

    fn check_destructive(
        &self,
        tool_name: &str,
        tool_input: &Value,
    ) -> Option<(GateAction, String)> {
        // Check bash commands
        if tool_name == "Bash" {
            if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
                let cmd_lower = cmd.to_lowercase();
                for (regex, desc, risk) in &self.destructive_patterns {
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

    fn check_secrets(&self, tool_input: &Value) -> Option<(GateAction, String)> {
        let input_str = tool_input.to_string();
        for regex in &self.secret_patterns {
            if regex.is_match(&input_str) {
                return Some((
                    GateAction::Deny,
                    "[secrets] Potential secret/credential detected in tool input".to_string(),
                ));
            }
        }
        None
    }

    fn check_file_scope(
        &self,
        tool_name: &str,
        tool_input: &Value,
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

        for protected in &self.protected_paths {
            if Self::glob_match(protected, file_path) {
                return Some((
                    GateAction::Deny,
                    format!(
                        "[file_scope] Write to protected path: {file_path} (matches {protected})"
                    ),
                ));
            }
        }

        None
    }

    fn check_custom_rule(
        &self,
        rule: &CompiledRule,
        tool_name: &str,
        tool_input: &Value,
    ) -> Option<(GateAction, String)> {
        let text = match rule.scope {
            crate::types::RuleScope::Tool => tool_name.to_string(),
            crate::types::RuleScope::Command => tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            crate::types::RuleScope::File => tool_input
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

    fn check_diff_size(&self, tool_name: &str, tool_input: &Value) -> Option<(GateAction, String)> {
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
        if lines > self.config.max_diff_lines {
            return Some((
                GateAction::Ask,
                format!(
                    "[diff_size] Edit changes ~{lines} lines (max: {})",
                    self.config.max_diff_lines
                ),
            ));
        }

        None
    }

    fn build_destructive_patterns() -> Vec<(Regex, &'static str, RiskLevel)> {
        let patterns: Vec<(&str, &str, RiskLevel)> = vec![
            (
                r"rm\s+-rf\s+[/~]",
                "Recursive delete of root/home",
                RiskLevel::Critical,
            ),
            (
                r"rm\s+-rf\s+/\*",
                "Recursive delete of all root contents",
                RiskLevel::Critical,
            ),
            (r":\(\)\{:\|:&\};:", "Fork bomb", RiskLevel::Critical),
            (r"mkfs\.", "Filesystem formatting", RiskLevel::Critical),
            (
                r"dd\s+if=/dev/(zero|random|urandom)",
                "Disk overwrite",
                RiskLevel::Critical,
            ),
            (
                r">\s*/dev/sd[a-z]",
                "Direct disk overwrite",
                RiskLevel::Critical,
            ),
            (
                r"chmod\s+-R\s+777\s+/",
                "Remove permissions from root",
                RiskLevel::Critical,
            ),
            (
                r"--no-preserve-root",
                "Root protection bypass",
                RiskLevel::Critical,
            ),
            (
                r"sudo\s+rm\s+-rf",
                "Sudo recursive force delete",
                RiskLevel::Critical,
            ),
            (r"git\s+reset\s+--hard", "Git hard reset", RiskLevel::High),
            (r"git\s+push\s+--force", "Git force push", RiskLevel::High),
            (r"git\s+push\s+-f\b", "Git force push", RiskLevel::High),
            (r"git\s+clean\s+-fd", "Git clean force", RiskLevel::High),
            (
                r"(wget|curl)\s.*\|\s*(ba)?sh",
                "Pipe download to shell",
                RiskLevel::High,
            ),
        ];

        patterns
            .into_iter()
            .filter_map(|(pat, desc, risk)| Regex::new(pat).ok().map(|r| (r, desc, risk)))
            .collect()
    }

    fn build_secret_patterns() -> Vec<Regex> {
        let patterns = [
            r"AKIA[0-9A-Z]{16}",                         // AWS access key
            r"(?i)bearer\s+[a-z0-9\-._~+/]+=*",          // Bearer token
            r"-----BEGIN\s+(RSA\s+)?PRIVATE\s+KEY-----", // Private key
            r#"(?i)["']?(api[_-]?key|api[_-]?secret|access[_-]?token|auth[_-]?token|secret[_-]?key)["']?\s*[:=]\s*["'][a-z0-9]{20,}"#, // Generic API keys
        ];

        patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
    }

    /// Simple glob matching for protected paths.
    fn glob_match(pattern: &str, path: &str) -> bool {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GuidanceConfig;
    use crate::types::{GateAction, GuidanceRule, RiskLevel, RuleScope};
    use serde_json::json;

    fn default_engine() -> GuidanceEngine {
        GuidanceEngine::from_config(&GuidanceConfig::default()).unwrap()
    }

    #[test]
    fn test_destructive_ops_gate_blocks_rm_rf() {
        let engine = default_engine();
        let input = json!({"command": "rm -rf /"});
        let (action, reason, _) = engine.evaluate("Bash", &input, 0.0);
        assert_eq!(action, GateAction::Deny);
        assert!(reason.contains("destructive_ops"));
    }

    #[test]
    fn test_destructive_ops_gate_allows_safe_command() {
        let engine = default_engine();
        let input = json!({"command": "ls -la"});
        let (action, _, _) = engine.evaluate("Bash", &input, 0.0);
        assert_eq!(action, GateAction::Allow);
    }

    #[test]
    fn test_secrets_gate_detects_aws_key() {
        let engine = default_engine();
        let input = json!({"content": "AKIAIOSFODNN7EXAMPLE"});
        let (action, reason, _) = engine.evaluate("Write", &input, 0.0);
        assert_eq!(action, GateAction::Deny);
        assert!(reason.contains("secrets"));
    }

    #[test]
    fn test_secrets_gate_allows_normal_text() {
        let engine = default_engine();
        let input = json!({"content": "Hello world"});
        let (action, _, _) = engine.evaluate("Write", &input, 0.0);
        assert_eq!(action, GateAction::Allow);
    }

    #[test]
    fn test_file_scope_gate_blocks_protected_path() {
        let engine = default_engine();
        let input = json!({"file_path": "/project/.env"});
        let (action, reason, _) = engine.evaluate("Write", &input, 0.0);
        assert_eq!(action, GateAction::Deny);
        assert!(reason.contains("file_scope"));
    }

    #[test]
    fn test_file_scope_gate_allows_normal_file() {
        let engine = default_engine();
        let input = json!({"file_path": "/project/src/main.rs"});
        let (action, _, _) = engine.evaluate("Write", &input, 0.0);
        assert_eq!(action, GateAction::Allow);
    }

    #[test]
    fn test_custom_rule_deny() {
        let config = GuidanceConfig {
            custom_rules: vec![GuidanceRule {
                id: "no-npm".to_string(),
                pattern: r"npm\s+install".to_string(),
                action: GateAction::Deny,
                scope: RuleScope::Command,
                risk_level: RiskLevel::Medium,
                description: "No npm install".to_string(),
                enabled: true,
            }],
            ..Default::default()
        };
        let engine = GuidanceEngine::from_config(&config).unwrap();
        let input = json!({"command": "npm install foo"});
        let (action, _, rule_id) = engine.evaluate("Bash", &input, 0.0);
        assert_eq!(action, GateAction::Deny);
        assert_eq!(rule_id, Some("no-npm".to_string()));
    }

    #[test]
    fn test_trust_relaxation_promotes_ask_to_allow() {
        let engine = default_engine();
        // git reset --hard triggers Ask at High risk
        let input = json!({"command": "git reset --hard HEAD"});
        let (action_low_trust, _, _) = engine.evaluate("Bash", &input, 0.0);
        assert_eq!(action_low_trust, GateAction::Deny); // High risk → Deny

        // git force push is also Deny, so let's test with diff_size which produces Ask
        let mut config = GuidanceConfig::default();
        config.max_diff_lines = 10;
        let engine = GuidanceEngine::from_config(&config).unwrap();
        let big_content = "line\n".repeat(50);
        let input = json!({"content": big_content});
        let (action_low, _, _) = engine.evaluate("Write", &input, 0.0);
        assert_eq!(action_low, GateAction::Ask);
        let (action_high, _, _) = engine.evaluate("Write", &input, 0.9);
        assert_eq!(action_high, GateAction::Allow); // Trust relaxation
    }

    #[test]
    fn test_diff_size_gate() {
        let mut config = GuidanceConfig::default();
        config.max_diff_lines = 5;
        let engine = GuidanceEngine::from_config(&config).unwrap();
        let input = json!({"content": "a\nb\nc\nd\ne\nf\ng\nh"});
        let (action, reason, _) = engine.evaluate("Write", &input, 0.0);
        assert_eq!(action, GateAction::Ask);
        assert!(reason.contains("diff_size"));
    }
}
