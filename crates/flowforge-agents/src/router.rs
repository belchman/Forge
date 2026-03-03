use std::collections::HashMap;

use flowforge_core::config::RoutingConfig;
use flowforge_core::{AgentDef, RoutingBreakdown, RoutingResult};
use regex::Regex;
use tracing::warn;

/// Routes tasks to the best-matching agent based on a weighted scoring algorithm.
///
/// Score = pattern_weight * pattern_score
///       + capability_weight * capability_score
///       + learned_weight * learned_score
///       + priority_weight * priority_score
pub struct AgentRouter {
    pattern_weight: f64,
    capability_weight: f64,
    learned_weight: f64,
    priority_weight: f64,
}

impl AgentRouter {
    /// Create a new router with the given weight configuration.
    pub fn new(config: &RoutingConfig) -> Self {
        Self {
            pattern_weight: config.pattern_weight,
            capability_weight: config.capability_weight,
            learned_weight: config.learned_weight,
            priority_weight: config.priority_weight,
        }
    }

    /// Route a task to the best-matching agents.
    ///
    /// Returns a list of `RoutingResult` sorted by confidence (highest first).
    ///
    /// - `task`: the task description text
    /// - `agents`: available agent definitions to score
    /// - `learned_weights`: mapping of (task_pattern, agent_name) -> weight from learning system
    pub fn route(
        &self,
        task: &str,
        agents: &[&AgentDef],
        learned_weights: &HashMap<(String, String), f64>,
    ) -> Vec<RoutingResult> {
        let mut results: Vec<RoutingResult> = agents
            .iter()
            .map(|agent| {
                let pattern_score = self.compute_pattern_score(task, agent);
                let capability_score = self.compute_capability_score(task, agent);
                let learned_score = self.compute_learned_score(task, agent, learned_weights);
                let priority_score = agent.priority.boost();

                let confidence = self.pattern_weight * pattern_score
                    + self.capability_weight * capability_score
                    + self.learned_weight * learned_score
                    + self.priority_weight * priority_score;

                RoutingResult {
                    agent_name: agent.name.clone(),
                    confidence,
                    breakdown: RoutingBreakdown {
                        pattern_score,
                        capability_score,
                        learned_score,
                        priority_score,
                    },
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Check agent's regex patterns against the task text.
    /// Returns the fraction of patterns that match (0.0 to 1.0).
    /// Patterns are automatically made case-insensitive. A leading word boundary
    /// is added to each alternative to prevent mid-word matches (e.g. "sync"
    /// won't match inside "async"), while allowing suffix variations (e.g.
    /// "document" matches "documentation").
    fn compute_pattern_score(&self, task: &str, agent: &AgentDef) -> f64 {
        if agent.patterns.is_empty() {
            return 0.0;
        }

        let mut matches = 0usize;
        for pattern_str in &agent.patterns {
            // Add leading word boundary if pattern doesn't already use anchors/boundaries
            let wrapped = if pattern_str.contains("\\b")
                || pattern_str.starts_with('^')
                || pattern_str.ends_with('$')
            {
                format!("(?i){pattern_str}")
            } else {
                // Wrap each alternative with a leading \b to prevent mid-word matches
                let bounded = pattern_str
                    .split('|')
                    .map(|alt| format!("\\b(?:{alt})"))
                    .collect::<Vec<_>>()
                    .join("|");
                format!("(?i){bounded}")
            };

            match Regex::new(&wrapped) {
                Ok(re) => {
                    if re.is_match(task) {
                        matches += 1;
                    }
                }
                Err(e) => {
                    warn!(
                        "Invalid regex pattern '{}' for agent '{}': {e}",
                        pattern_str, agent.name
                    );
                }
            }
        }

        matches as f64 / agent.patterns.len() as f64
    }

    /// Count keyword overlap between task words and agent capabilities.
    /// Normalized by the number of capabilities (max possible matches).
    fn compute_capability_score(&self, task: &str, agent: &AgentDef) -> f64 {
        if agent.capabilities.is_empty() {
            return 0.0;
        }

        let task_lower = task.to_lowercase();
        let task_words: Vec<&str> = task_lower.split_whitespace().collect();

        let matches = agent
            .capabilities
            .iter()
            .filter(|cap| {
                let cap_lower = cap.to_lowercase();
                // Check if the capability appears as a word or substring in the task
                task_words
                    .iter()
                    .any(|word| word.contains(&cap_lower) || cap_lower.contains(word))
            })
            .count();

        matches as f64 / agent.capabilities.len() as f64
    }

    /// Look up learned routing weights for this task/agent pair.
    /// Returns 0.5 as default if no learned weight is found.
    fn compute_learned_score(
        &self,
        task: &str,
        agent: &AgentDef,
        learned_weights: &HashMap<(String, String), f64>,
    ) -> f64 {
        // Check for exact or pattern-based matches in the learned weights
        for ((task_pattern, agent_name), weight) in learned_weights {
            if agent_name == &agent.name {
                match Regex::new(task_pattern) {
                    Ok(re) => {
                        if re.is_match(task) {
                            return *weight;
                        }
                    }
                    Err(_) => {
                        // Fall back to substring matching if the pattern isn't valid regex
                        if task.contains(task_pattern.as_str()) {
                            return *weight;
                        }
                    }
                }
            }
        }
        0.5 // default when no learned weight found
    }
}

impl Default for AgentRouter {
    fn default() -> Self {
        Self::new(&RoutingConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowforge_core::{AgentSource, Priority};

    fn make_agent(name: &str, caps: &[&str], patterns: &[&str], priority: Priority) -> AgentDef {
        AgentDef {
            name: name.to_string(),
            description: String::new(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            patterns: patterns.iter().map(|s| s.to_string()).collect(),
            priority,
            color: None,
            body: String::new(),
            source: AgentSource::BuiltIn,
        }
    }

    #[test]
    fn test_route_pattern_match() {
        let router = AgentRouter::default();
        // Single pattern — should score 1.0 when it matches
        let agent = make_agent("tester", &["test"], &["test"], Priority::Normal);
        let agents: Vec<&AgentDef> = vec![&agent];
        let learned = HashMap::new();

        let results = router.route("test the login flow", &agents, &learned);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].breakdown.pattern_score, 1.0);
    }

    #[test]
    fn test_route_partial_pattern_match() {
        let router = AgentRouter::default();
        // Two patterns — only "test" matches, "spec" doesn't → 0.5
        let agent = make_agent("tester", &["test"], &["test", "spec"], Priority::Normal);
        let agents: Vec<&AgentDef> = vec![&agent];
        let learned = HashMap::new();

        let results = router.route("test the login flow", &agents, &learned);
        assert!((results[0].breakdown.pattern_score - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_route_no_pattern_match() {
        let router = AgentRouter::default();
        let agent = make_agent("tester", &["test"], &["^deploy.*"], Priority::Normal);
        let agents: Vec<&AgentDef> = vec![&agent];
        let learned = HashMap::new();

        let results = router.route("test the login flow", &agents, &learned);
        assert_eq!(results[0].breakdown.pattern_score, 0.0);
    }

    #[test]
    fn test_route_word_boundary() {
        let router = AgentRouter::default();
        // "sync" should NOT match "async" — leading \b prevents mid-word matches
        let sync_agent = make_agent("syncer", &["sync"], &["sync"], Priority::Normal);
        let agents: Vec<&AgentDef> = vec![&sync_agent];
        let learned = HashMap::new();

        let results = router.route("fix async handler", &agents, &learned);
        assert_eq!(results[0].breakdown.pattern_score, 0.0);

        // But should match when "sync" is a standalone word
        let results = router.route("sync the database", &agents, &learned);
        assert_eq!(results[0].breakdown.pattern_score, 1.0);

        // "document" should match "documentation" (leading boundary, suffix allowed)
        let doc_agent = make_agent("doc", &[], &["document"], Priority::Normal);
        let agents: Vec<&AgentDef> = vec![&doc_agent];
        let results = router.route("write documentation", &agents, &learned);
        assert_eq!(results[0].breakdown.pattern_score, 1.0);
    }

    #[test]
    fn test_route_capability_score() {
        let router = AgentRouter::default();
        let agent = make_agent(
            "reviewer",
            &["rust", "review", "lint"],
            &[],
            Priority::Normal,
        );
        let agents: Vec<&AgentDef> = vec![&agent];
        let learned = HashMap::new();

        let results = router.route("review this rust code", &agents, &learned);
        let cap_score = results[0].breakdown.capability_score;
        // "rust" and "review" should match out of 3 capabilities
        assert!((cap_score - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_route_sorting() {
        let router = AgentRouter::default();
        let low = make_agent("low", &[], &[], Priority::Low);
        let high = make_agent("high", &["test"], &["test.*"], Priority::High);
        let agents: Vec<&AgentDef> = vec![&low, &high];
        let learned = HashMap::new();

        let results = router.route("test something", &agents, &learned);
        assert_eq!(results[0].agent_name, "high");
        assert_eq!(results[1].agent_name, "low");
    }

    #[test]
    fn test_route_learned_weights() {
        let router = AgentRouter::default();
        let agent_a = make_agent("agent-a", &[], &[], Priority::Normal);
        let agent_b = make_agent("agent-b", &[], &[], Priority::Normal);
        let agents: Vec<&AgentDef> = vec![&agent_a, &agent_b];

        let mut learned = HashMap::new();
        learned.insert(("deploy".to_string(), "agent-b".to_string()), 0.9);

        let results = router.route("deploy the service", &agents, &learned);
        // agent-b should rank higher due to the learned weight
        assert_eq!(results[0].agent_name, "agent-b");
    }

    #[test]
    fn test_route_empty_agents() {
        let router = AgentRouter::default();
        let agents: Vec<&AgentDef> = vec![];
        let learned = HashMap::new();

        let results = router.route("anything", &agents, &learned);
        assert!(results.is_empty());
    }
}
