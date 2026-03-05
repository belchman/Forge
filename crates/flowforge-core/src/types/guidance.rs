use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "critical"),
            Self::High => write!(f, "high"),
            Self::Medium => write!(f, "medium"),
            Self::Low => write!(f, "low"),
        }
    }
}

impl std::str::FromStr for RiskLevel {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            other => Err(format!("unknown risk level: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateAction {
    Deny,
    Ask,
    Allow,
}

impl std::fmt::Display for GateAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deny => write!(f, "deny"),
            Self::Ask => write!(f, "ask"),
            Self::Allow => write!(f, "allow"),
        }
    }
}

impl std::str::FromStr for GateAction {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "deny" => Ok(Self::Deny),
            "ask" => Ok(Self::Ask),
            "allow" => Ok(Self::Allow),
            other => Err(format!("unknown gate action: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleScope {
    Tool,
    Command,
    File,
}

impl std::fmt::Display for RuleScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tool => write!(f, "tool"),
            Self::Command => write!(f, "command"),
            Self::File => write!(f, "file"),
        }
    }
}

impl std::str::FromStr for RuleScope {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "tool" => Ok(Self::Tool),
            "command" => Ok(Self::Command),
            "file" => Ok(Self::File),
            other => Err(format!("unknown rule scope: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceRule {
    pub id: String,
    pub pattern: String,
    pub action: GateAction,
    pub scope: RuleScope,
    pub risk_level: RiskLevel,
    pub description: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    pub id: i64,
    pub session_id: String,
    pub rule_id: Option<String>,
    pub gate_name: String,
    pub tool_name: String,
    pub action: GateAction,
    pub reason: String,
    pub risk_level: RiskLevel,
    pub trust_before: f64,
    pub trust_after: f64,
    pub timestamp: DateTime<Utc>,
    pub hash: String,
    pub prev_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScore {
    pub session_id: String,
    pub score: f64,
    pub total_checks: u64,
    pub denials: u64,
    pub asks: u64,
    pub allows: u64,
    pub last_updated: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
