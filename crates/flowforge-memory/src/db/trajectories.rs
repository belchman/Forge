use chrono::Utc;
use rusqlite::params;

use flowforge_core::{
    trajectory::{StepOutcome, Trajectory, TrajectoryStatus, TrajectoryStep, TrajectoryVerdict},
    Result,
};

use super::row_parsers::parse_trajectory_row;
use super::{parse_datetime, MemoryDb, SqliteExt};

use rusqlite::OptionalExtension;

impl MemoryDb {
    // ── Trajectories ──

    pub fn create_trajectory(&self, trajectory: &Trajectory) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO trajectories
                 (id, session_id, work_item_id, agent_name, task_description, status,
                  started_at, ended_at, verdict, confidence, metadata, embedding_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    trajectory.id,
                    trajectory.session_id,
                    trajectory.work_item_id,
                    trajectory.agent_name,
                    trajectory.task_description,
                    trajectory.status.to_string(),
                    trajectory.started_at.to_rfc3339(),
                    trajectory.ended_at.map(|t| t.to_rfc3339()),
                    trajectory.verdict.map(|v| v.to_string()),
                    trajectory.confidence,
                    trajectory.metadata,
                    trajectory.embedding_id,
                ],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_trajectory(&self, id: &str) -> Result<Option<Trajectory>> {
        self.conn
            .query_row(
                "SELECT id, session_id, work_item_id, agent_name, task_description, status,
                        started_at, ended_at, verdict, confidence, metadata, embedding_id
                 FROM trajectories WHERE id = ?1",
                params![id],
                |row| Ok(parse_trajectory_row(row)),
            )
            .optional()
            .sq()
    }

    pub fn get_active_trajectory(&self, session_id: &str) -> Result<Option<Trajectory>> {
        self.conn
            .query_row(
                "SELECT id, session_id, work_item_id, agent_name, task_description, status,
                        started_at, ended_at, verdict, confidence, metadata, embedding_id
                 FROM trajectories WHERE session_id = ?1 AND status = 'recording'
                 ORDER BY started_at DESC LIMIT 1",
                params![session_id],
                |row| Ok(parse_trajectory_row(row)),
            )
            .optional()
            .sq()
    }

    pub fn end_trajectory(&self, id: &str, status: TrajectoryStatus) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE trajectories SET ended_at = ?1, status = ?2 WHERE id = ?3",
                params![now, status.to_string(), id],
            )
            .sq()?;
        Ok(())
    }

    pub fn judge_trajectory(
        &self,
        id: &str,
        verdict: TrajectoryVerdict,
        confidence: f64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE trajectories SET status = 'judged', verdict = ?1, confidence = ?2, ended_at = COALESCE(ended_at, ?3)
                 WHERE id = ?4",
                params![verdict.to_string(), confidence, now, id],
            )
            .sq()?;
        Ok(())
    }

    pub fn list_trajectories(
        &self,
        session_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Trajectory>> {
        let mut sql = String::from(
            "SELECT id, session_id, work_item_id, agent_name, task_description, status,
                    started_at, ended_at, verdict, confidence, metadata, embedding_id
             FROM trajectories WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(sid) = session_id {
            param_values.push(Box::new(sid.to_string()));
            sql.push_str(&format!(" AND session_id = ?{}", param_values.len()));
        }
        if let Some(st) = status {
            param_values.push(Box::new(st.to_string()));
            sql.push_str(&format!(" AND status = ?{}", param_values.len()));
        }
        param_values.push(Box::new(limit as i64));
        sql.push_str(&format!(
            " ORDER BY started_at DESC LIMIT ?{}",
            param_values.len()
        ));

        let mut stmt = self.conn.prepare(&sql).sq()?;
        let params_slice: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(params_slice.as_slice(), |row| Ok(parse_trajectory_row(row)))
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn set_trajectory_task_description(&self, id: &str, task_description: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE trajectories SET task_description = ?1 WHERE id = ?2",
                params![task_description, id],
            )
            .sq()?;
        Ok(())
    }

    pub fn set_trajectory_agent_name(&self, id: &str, agent_name: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE trajectories SET agent_name = ?1 WHERE id = ?2",
                params![agent_name, id],
            )
            .sq()?;
        Ok(())
    }

    pub fn link_trajectory_work_item(&self, trajectory_id: &str, work_item_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE trajectories SET work_item_id = ?1 WHERE id = ?2",
                params![work_item_id, trajectory_id],
            )
            .sq()?;
        Ok(())
    }

    // ── Trajectory Steps ──

    pub fn record_trajectory_step(
        &self,
        trajectory_id: &str,
        tool_name: &str,
        tool_input_hash: Option<&str>,
        outcome: StepOutcome,
        duration_ms: Option<i64>,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO trajectory_steps
                 (trajectory_id, step_index, tool_name, tool_input_hash, outcome, duration_ms, timestamp)
                 VALUES (?1, (SELECT COALESCE(MAX(step_index), -1) + 1 FROM trajectory_steps WHERE trajectory_id = ?1),
                         ?2, ?3, ?4, ?5, ?6)",
                params![
                    trajectory_id,
                    tool_name,
                    tool_input_hash,
                    outcome.to_string(),
                    duration_ms,
                    now,
                ],
            )
            .sq()?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_trajectory_steps(&self, trajectory_id: &str) -> Result<Vec<TrajectoryStep>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, trajectory_id, step_index, tool_name, tool_input_hash, outcome, duration_ms, timestamp
                 FROM trajectory_steps WHERE trajectory_id = ?1 ORDER BY step_index ASC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![trajectory_id], |row| {
                Ok(TrajectoryStep {
                    id: row.get(0)?,
                    trajectory_id: row.get(1)?,
                    step_index: row.get(2)?,
                    tool_name: row.get(3)?,
                    tool_input_hash: row.get(4)?,
                    outcome: row
                        .get::<_, String>(5)?
                        .parse()
                        .unwrap_or(StepOutcome::Success),
                    duration_ms: row.get(6)?,
                    timestamp: parse_datetime(row.get::<_, String>(7)?),
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn trajectory_success_ratio(&self, trajectory_id: &str) -> Result<f64> {
        let (total, successes): (i64, i64) = self
            .conn
            .query_row(
                "SELECT COUNT(*), SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END)
                 FROM trajectory_steps WHERE trajectory_id = ?1",
                params![trajectory_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .sq()?;
        if total == 0 {
            return Ok(0.0);
        }
        Ok(successes as f64 / total as f64)
    }

    pub fn trajectory_tool_sequence(&self, trajectory_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT tool_name FROM trajectory_steps WHERE trajectory_id = ?1 ORDER BY step_index ASC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![trajectory_id], |row| row.get(0))
            .sq()?;
        rows.collect::<std::result::Result<Vec<String>, _>>().sq()
    }

    pub fn delete_old_failed_trajectories(&self, older_than_days: u64) -> Result<u32> {
        let threshold = Utc::now() - chrono::Duration::days(older_than_days as i64);
        // Steps are cascade-deleted via FK
        let count = self
            .conn
            .execute(
                "DELETE FROM trajectories WHERE status = 'failed' AND started_at < ?1",
                params![threshold.to_rfc3339()],
            )
            .sq()?;
        Ok(count as u32)
    }

    pub fn count_trajectories_by_status(&self) -> Result<Vec<(String, u64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT status, COUNT(*) FROM trajectories GROUP BY status")
            .sq()?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }
}
