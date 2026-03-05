//! Guidance Control Plane: rule engine for evaluating tool uses.

pub(crate) mod gates;
pub mod patterns;
mod tests;

use regex::Regex;
use serde_json::Value;

use crate::config::GuidanceConfig;
use crate::types::{GateAction, RuleScope};
use crate::Result;

/// Compiled guidance engine with regex patterns for all gates.
pub struct GuidanceEngine {
    protected_paths: Vec<String>,
    custom_rules: Vec<CompiledRule>,
    config: GuidanceConfig,
}

pub(crate) struct CompiledRule {
    pub(crate) id: String,
    pub(crate) regex: Regex,
    pub(crate) action: GateAction,
    pub(crate) scope: RuleScope,
    pub(crate) description: String,
}

impl GuidanceEngine {
    /// Build the engine from config. Only compiles custom rules (built-in patterns use LazyLock).
    pub fn from_config(config: &GuidanceConfig) -> Result<Self> {
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
            if let Some((action, reason)) = gates::check_destructive(tool_name, tool_input) {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        // Gate 2: Secrets detection
        if self.config.secrets_gate {
            if let Some((action, reason)) = gates::check_secrets(tool_input) {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        // Gate 3: File scope
        if self.config.file_scope_gate {
            if let Some((action, reason)) =
                gates::check_file_scope(tool_name, tool_input, &self.protected_paths)
            {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        // Gate 4: Custom rules
        for rule in &self.custom_rules {
            if let Some((action, reason)) = gates::check_custom_rule(rule, tool_name, tool_input) {
                return self.apply_trust(action, reason, Some(rule.id.clone()), trust);
            }
        }

        // Gate 5: Diff size
        if self.config.diff_size_gate {
            if let Some((action, reason)) =
                gates::check_diff_size(tool_name, tool_input, &self.config)
            {
                return self.apply_trust(action, reason, None, trust);
            }
        }

        // Baseline trust check: if trust is very low, ask even when all gates pass
        if trust < self.config.trust_deny_threshold {
            return (
                GateAction::Ask,
                format!(
                    "low session trust ({trust:.2} < {:.2})",
                    self.config.trust_deny_threshold
                ),
                None,
            );
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
}
