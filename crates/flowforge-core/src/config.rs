use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main FlowForge configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlowForgeConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub routing: RoutingConfig,
    #[serde(default)]
    pub patterns: PatternsConfig,
    #[serde(default)]
    pub tmux: TmuxConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub work_tracking: WorkTrackingConfig,
    #[serde(default)]
    pub guidance: GuidanceConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub log_level: String,
    pub telemetry: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            telemetry: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub db_name: String,
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub hnsw_ef_search: usize,
    pub embedding_dim: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            db_name: "flowforge.db".to_string(),
            hnsw_m: 16,
            hnsw_ef_construction: 100,
            hnsw_ef_search: 50,
            embedding_dim: 128,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    pub load_builtin: bool,
    pub load_global: bool,
    pub load_project: bool,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            load_builtin: true,
            load_global: true,
            load_project: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    pub pattern_weight: f64,
    pub capability_weight: f64,
    pub learned_weight: f64,
    pub priority_weight: f64,
    pub context_weight: f64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            pattern_weight: 0.35,
            capability_weight: 0.25,
            learned_weight: 0.20,
            priority_weight: 0.05,
            context_weight: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PatternsConfig {
    pub short_term_max: usize,
    pub short_term_ttl_hours: u64,
    pub long_term_max: usize,
    pub promotion_min_usage: u32,
    pub promotion_min_confidence: f64,
    pub decay_rate_per_hour: f64,
    pub dedup_similarity_threshold: f64,
    pub trajectory_max: usize,
    pub trajectory_prune_days: u64,
    pub trajectory_merge_threshold: f64,
    pub semantic_embeddings: bool,
    pub clustering_min_points: usize,
    pub clustering_epsilon: f64,
    pub outlier_recluster_threshold: usize,
    pub cluster_decay_active_factor: f64,
    pub cluster_decay_isolated_factor: f64,
    /// Minimum similarity score for pattern injection (filters noise).
    pub min_injection_similarity: f64,
    /// Fraction of patterns to withhold for A/B testing (0.0 = none).
    #[serde(default)]
    pub ab_test_holdout_rate: f64,
}

impl Default for PatternsConfig {
    fn default() -> Self {
        Self {
            short_term_max: 500,
            short_term_ttl_hours: 24,
            long_term_max: 2000,
            promotion_min_usage: 3,
            promotion_min_confidence: 0.6,
            decay_rate_per_hour: 0.005,
            dedup_similarity_threshold: 0.88,
            trajectory_max: 5000,
            trajectory_prune_days: 7,
            trajectory_merge_threshold: 0.9,
            semantic_embeddings: true,
            clustering_min_points: 2,
            clustering_epsilon: 0.5,
            outlier_recluster_threshold: 50,
            cluster_decay_active_factor: 0.5,
            cluster_decay_isolated_factor: 2.0,
            min_injection_similarity: 0.55,
            ab_test_holdout_rate: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TmuxConfig {
    pub session_name: String,
    pub auto_start: bool,
    pub refresh_interval_ms: u64,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        Self {
            session_name: "flowforge".to_string(),
            auto_start: true,
            refresh_interval_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksConfig {
    pub bash_validation: bool,
    pub edit_tracking: bool,
    pub routing: bool,
    pub learning: bool,
    /// If true, inject the full agent markdown body instead of a 1-line summary.
    pub inject_agent_body: bool,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            bash_validation: true,
            edit_tracking: true,
            routing: true,
            learning: true,
            inject_agent_body: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkTrackingConfig {
    pub backend: String,
    pub log_all: bool,
    pub require_task: bool,
    /// Block mutating tool calls when no active work item exists.
    /// Toggle off with `enforce_gate = false` or `FLOWFORGE_NO_WORK_GATE=1`.
    pub enforce_gate: bool,
    pub kanbus: KanbusSyncConfig,
    pub beads: BeadsSyncConfig,
    pub claude_tasks: ClaudeTasksSyncConfig,
    pub work_stealing: WorkStealingConfig,
}

impl Default for WorkTrackingConfig {
    fn default() -> Self {
        Self {
            backend: "auto".to_string(),
            log_all: true,
            require_task: false,
            enforce_gate: true,
            kanbus: KanbusSyncConfig::default(),
            beads: BeadsSyncConfig::default(),
            claude_tasks: ClaudeTasksSyncConfig::default(),
            work_stealing: WorkStealingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkStealingConfig {
    pub enabled: bool,
    pub stale_threshold_mins: u64,
    pub abandon_threshold_mins: u64,
    pub stale_min_progress: i32,
    pub max_steal_count: u32,
    pub steal_cooldown_mins: u64,
    pub max_concurrent_claims: u64,
    pub scan_interval_mins: u64,
}

impl Default for WorkStealingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            stale_threshold_mins: 30,
            abandon_threshold_mins: 60,
            stale_min_progress: 25,
            max_steal_count: 3,
            steal_cooldown_mins: 10,
            max_concurrent_claims: 5,
            scan_interval_mins: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KanbusSyncConfig {
    pub project_key: Option<String>,
    pub cli_command: String,
    pub root: Option<std::path::PathBuf>,
}

impl Default for KanbusSyncConfig {
    fn default() -> Self {
        Self {
            project_key: None,
            cli_command: "kbs".to_string(),
            root: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BeadsSyncConfig {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClaudeTasksSyncConfig {
    pub list_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GuidanceConfig {
    pub enabled: bool,
    pub destructive_ops_gate: bool,
    pub file_scope_gate: bool,
    pub diff_size_gate: bool,
    pub secrets_gate: bool,
    pub max_diff_lines: usize,
    pub trust_initial_score: f64,
    pub trust_ask_threshold: f64,
    pub trust_decay_per_hour: f64,
    pub protected_paths: Vec<String>,
    pub custom_rules: Vec<crate::types::GuidanceRule>,
    /// Tools that bypass guidance gates entirely (read-only, safe tools).
    pub safe_tools: Vec<String>,
}

impl Default for GuidanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            destructive_ops_gate: true,
            file_scope_gate: true,
            diff_size_gate: true,
            secrets_gate: true,
            max_diff_lines: 500,
            trust_initial_score: 0.5,
            trust_ask_threshold: 0.8,
            trust_decay_per_hour: 0.02,
            protected_paths: vec![],
            custom_rules: vec![],
            safe_tools: vec![
                "Read",
                "Glob",
                "Grep",
                "LSP",
                "WebSearch",
                "WebFetch",
                "TaskList",
                "TaskGet",
                "TaskCreate",
                "TaskUpdate",
                "ToolSearch",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub disabled: Vec<String>,
}

impl FlowForgeConfig {
    /// Load config from a TOML file, falling back to defaults
    pub fn load(path: &Path) -> crate::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: Self = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to a TOML file
    pub fn save(&self, path: &Path) -> crate::Result<()> {
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate config for common misconfigurations
    pub fn validate(&self) -> crate::Result<()> {
        let ws = &self.work_tracking.work_stealing;
        if ws.abandon_threshold_mins <= ws.stale_threshold_mins {
            return Err(crate::Error::Config(
                "abandon_threshold_mins must be greater than stale_threshold_mins".to_string(),
            ));
        }
        if self.guidance.trust_initial_score > 1.0 || self.guidance.trust_initial_score < 0.0 {
            return Err(crate::Error::Config(
                "trust_initial_score must be between 0.0 and 1.0".to_string(),
            ));
        }
        if ws.max_concurrent_claims == 0 {
            return Err(crate::Error::Config(
                "max_concurrent_claims must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Get project .flowforge directory
    pub fn project_dir() -> PathBuf {
        PathBuf::from(".flowforge")
    }

    /// Get global ~/.flowforge directory
    pub fn global_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".flowforge")
    }

    /// Get the database path for a project
    pub fn db_path(&self) -> PathBuf {
        Self::project_dir().join(&self.memory.db_name)
    }

    /// Get the config file path for a project
    pub fn config_path() -> PathBuf {
        Self::project_dir().join("config.toml")
    }

    /// Get the tmux state file path
    pub fn tmux_state_path() -> PathBuf {
        Self::project_dir().join("tmux-state.json")
    }

    /// Get the plugins directory
    pub fn plugins_dir() -> PathBuf {
        Self::project_dir().join("plugins")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_config() {
        let config = FlowForgeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_bad_abandon_threshold() {
        let mut config = FlowForgeConfig::default();
        config.work_tracking.work_stealing.abandon_threshold_mins = 10;
        config.work_tracking.work_stealing.stale_threshold_mins = 30;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_bad_trust_score() {
        let mut config = FlowForgeConfig::default();
        config.guidance.trust_initial_score = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_concurrent_claims() {
        let mut config = FlowForgeConfig::default();
        config.work_tracking.work_stealing.max_concurrent_claims = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_defaults_roundtrip() {
        let config = FlowForgeConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: FlowForgeConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.general.log_level, "info");
        assert_eq!(parsed.memory.hnsw_m, 16);
        assert!(parsed.guidance.enabled);
        assert_eq!(parsed.work_tracking.work_stealing.stale_threshold_mins, 30);
    }

    #[test]
    fn test_partial_toml_uses_defaults() {
        let toml_str = r#"
[general]
log_level = "debug"
"#;
        let config: FlowForgeConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.general.log_level, "debug");
        assert!(config.general.telemetry); // default true
        assert_eq!(config.memory.hnsw_m, 16); // default
        assert!(config.guidance.enabled); // default true
    }

    #[test]
    fn test_validate_negative_trust_score() {
        let mut config = FlowForgeConfig::default();
        config.guidance.trust_initial_score = -0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_equal_thresholds() {
        let mut config = FlowForgeConfig::default();
        config.work_tracking.work_stealing.abandon_threshold_mins = 30;
        config.work_tracking.work_stealing.stale_threshold_mins = 30;
        // equal is also invalid (must be strictly greater)
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_save_load_roundtrip() {
        let config = FlowForgeConfig::default();
        let dir = std::env::temp_dir().join("flowforge-test-config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        config.save(&path).unwrap();
        let loaded = FlowForgeConfig::load(&path).unwrap();
        assert_eq!(loaded.general.log_level, "info");
        assert_eq!(loaded.memory.hnsw_m, 16);
        assert!(loaded.guidance.enabled);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_config_load_nonexistent_returns_default() {
        let path = std::path::Path::new("/nonexistent/path/config.toml");
        let config = FlowForgeConfig::load(path).unwrap();
        assert_eq!(config.general.log_level, "info");
    }

    #[test]
    fn test_work_stealing_defaults() {
        let config = WorkStealingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.stale_threshold_mins, 30);
        assert_eq!(config.abandon_threshold_mins, 60);
        assert_eq!(config.max_steal_count, 3);
        assert_eq!(config.max_concurrent_claims, 5);
    }

    #[test]
    fn test_guidance_config_defaults() {
        let config = GuidanceConfig::default();
        assert!(config.destructive_ops_gate);
        assert!(config.file_scope_gate);
        assert!(config.secrets_gate);
        assert!(config.diff_size_gate);
        assert_eq!(config.max_diff_lines, 500);
        assert_eq!(config.trust_initial_score, 0.5);
        assert!(config.safe_tools.contains(&"Read".to_string()));
        assert!(config.safe_tools.contains(&"Grep".to_string()));
    }

    #[test]
    fn test_patterns_config_defaults() {
        let config = PatternsConfig::default();
        assert_eq!(config.short_term_max, 500);
        assert_eq!(config.long_term_max, 2000);
        assert_eq!(config.promotion_min_usage, 3);
        assert!((config.promotion_min_confidence - 0.6).abs() < f64::EPSILON);
        assert!((config.min_injection_similarity - 0.55).abs() < f64::EPSILON);
    }

    #[test]
    fn test_routing_config_weights_sum_to_one() {
        let config = RoutingConfig::default();
        let sum = config.pattern_weight
            + config.capability_weight
            + config.learned_weight
            + config.priority_weight
            + config.context_weight;
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_full_toml_parse() {
        let toml_str = r#"
[general]
log_level = "debug"
telemetry = false

[memory]
db_name = "custom.db"
hnsw_m = 32

[guidance]
enabled = false
max_diff_lines = 1000

[work_tracking]
backend = "kanbus"
enforce_gate = false

[work_tracking.work_stealing]
enabled = false
stale_threshold_mins = 60
abandon_threshold_mins = 120
"#;
        let config: FlowForgeConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.general.log_level, "debug");
        assert!(!config.general.telemetry);
        assert_eq!(config.memory.db_name, "custom.db");
        assert_eq!(config.memory.hnsw_m, 32);
        assert!(!config.guidance.enabled);
        assert_eq!(config.guidance.max_diff_lines, 1000);
        assert_eq!(config.work_tracking.backend, "kanbus");
        assert!(!config.work_tracking.enforce_gate);
        assert!(!config.work_tracking.work_stealing.enabled);
        assert_eq!(config.work_tracking.work_stealing.stale_threshold_mins, 60);
    }
}
