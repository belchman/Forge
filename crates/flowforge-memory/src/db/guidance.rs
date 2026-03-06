use chrono::Utc;
use rusqlite::params;

use flowforge_core::{
    types::{GateAction, GateDecision, RiskLevel, TrustScore},
    Result,
};

use super::{parse_datetime, MemoryDb, SqliteExt};

use rusqlite::OptionalExtension;

impl MemoryDb {
    // ── Trust Scores (Guidance) ──

    pub fn create_trust_score(&self, session_id: &str, initial_score: f64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT OR IGNORE INTO trust_scores (session_id, score, total_checks, denials, asks, allows, last_updated, created_at)
                 VALUES (?1, ?2, 0, 0, 0, 0, ?3, ?3)",
                params![session_id, initial_score, now],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_trust_score(&self, session_id: &str) -> Result<Option<TrustScore>> {
        self.conn
            .query_row(
                "SELECT session_id, score, total_checks, denials, asks, allows, last_updated, created_at
                 FROM trust_scores WHERE session_id = ?1",
                params![session_id],
                |row| {
                    Ok(TrustScore {
                        session_id: row.get(0)?,
                        score: row.get(1)?,
                        total_checks: row.get(2)?,
                        denials: row.get(3)?,
                        asks: row.get(4)?,
                        allows: row.get(5)?,
                        last_updated: parse_datetime(row.get::<_, String>(6)?),
                        created_at: parse_datetime(row.get::<_, String>(7)?),
                    })
                },
            )
            .optional()
            .sq()
    }

    /// Get the most recent trust score across all sessions (for carrying forward to new sessions).
    pub fn get_latest_trust_score(&self) -> Result<Option<f64>> {
        self.conn
            .query_row(
                "SELECT score FROM trust_scores ORDER BY last_updated DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .sq()
            .map(|o| o.flatten())
    }

    pub fn update_trust_score(
        &self,
        session_id: &str,
        action: &GateAction,
        trust_delta: f64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let (deny_inc, ask_inc, allow_inc) = match action {
            GateAction::Deny => (1, 0, 0),
            GateAction::Ask => (0, 1, 0),
            GateAction::Allow => (0, 0, 1),
        };
        self.conn
            .execute(
                "UPDATE trust_scores SET
                    score = MAX(0.0, MIN(1.0, score + ?1)),
                    total_checks = total_checks + 1,
                    denials = denials + ?2,
                    asks = asks + ?3,
                    allows = allows + ?4,
                    last_updated = ?5
                 WHERE session_id = ?6",
                params![trust_delta, deny_inc, ask_inc, allow_inc, now, session_id],
            )
            .sq()?;
        Ok(())
    }

    pub fn record_gate_decision(&self, decision: &GateDecision) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO gate_decisions
                 (session_id, rule_id, gate_name, tool_name, action, reason, risk_level,
                  trust_before, trust_after, timestamp, hash, prev_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    decision.session_id,
                    decision.rule_id,
                    decision.gate_name,
                    decision.tool_name,
                    decision.action.to_string(),
                    decision.reason,
                    decision.risk_level.to_string(),
                    decision.trust_before,
                    decision.trust_after,
                    decision.timestamp.to_rfc3339(),
                    decision.hash,
                    decision.prev_hash,
                ],
            )
            .sq()?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_gate_decisions(&self, session_id: &str, limit: usize) -> Result<Vec<GateDecision>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, rule_id, gate_name, tool_name, action, reason, risk_level,
                        trust_before, trust_after, timestamp, hash, prev_hash
                 FROM gate_decisions WHERE session_id = ?1
                 ORDER BY id DESC LIMIT ?2",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id, limit], |row| {
                Ok(GateDecision {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    rule_id: row.get(2)?,
                    gate_name: row.get(3)?,
                    tool_name: row.get(4)?,
                    action: row
                        .get::<_, String>(5)?
                        .parse()
                        .unwrap_or(GateAction::Allow),
                    reason: row.get(6)?,
                    risk_level: row.get::<_, String>(7)?.parse().unwrap_or(RiskLevel::Low),
                    trust_before: row.get(8)?,
                    trust_after: row.get(9)?,
                    timestamp: parse_datetime(row.get::<_, String>(10)?),
                    hash: row.get(11)?,
                    prev_hash: row.get(12)?,
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn get_gate_decisions_asc(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<GateDecision>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, rule_id, gate_name, tool_name, action, reason, risk_level,
                        trust_before, trust_after, timestamp, hash, prev_hash
                 FROM gate_decisions WHERE session_id = ?1
                 ORDER BY id ASC LIMIT ?2",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id, limit], |row| {
                Ok(GateDecision {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    rule_id: row.get(2)?,
                    gate_name: row.get(3)?,
                    tool_name: row.get(4)?,
                    action: row
                        .get::<_, String>(5)?
                        .parse()
                        .unwrap_or(GateAction::Allow),
                    reason: row.get(6)?,
                    risk_level: row.get::<_, String>(7)?.parse().unwrap_or(RiskLevel::Low),
                    trust_before: row.get(8)?,
                    trust_after: row.get(9)?,
                    timestamp: parse_datetime(row.get::<_, String>(10)?),
                    hash: row.get(11)?,
                    prev_hash: row.get(12)?,
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }
}
