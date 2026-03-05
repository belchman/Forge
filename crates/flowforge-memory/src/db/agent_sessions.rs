use chrono::Utc;
use rusqlite::params;

use flowforge_core::{AgentSession, AgentSessionStatus, Result};

use super::row_parsers::parse_agent_session_row;
use super::{MemoryDb, SqliteExt};

impl MemoryDb {
    pub fn create_agent_session(&self, session: &AgentSession) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO agent_sessions
                 (id, parent_session_id, agent_id, agent_type, status, started_at, ended_at, edits, commands, task_id, transcript_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    session.id,
                    session.parent_session_id,
                    session.agent_id,
                    session.agent_type,
                    session.status.to_string(),
                    session.started_at.to_rfc3339(),
                    session.ended_at.map(|t| t.to_rfc3339()),
                    session.edits,
                    session.commands,
                    session.task_id,
                    session.transcript_path,
                ],
            )
            .sq()?;
        Ok(())
    }

    pub fn end_agent_session(&self, agent_id: &str, status: AgentSessionStatus) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE agent_sessions SET ended_at = ?1, status = ?2
                 WHERE agent_id = ?3 AND ended_at IS NULL",
                params![now, status.to_string(), agent_id],
            )
            .sq()?;
        Ok(())
    }

    pub fn update_agent_session_status(
        &self,
        agent_id: &str,
        status: AgentSessionStatus,
    ) -> Result<()> {
        self.conn
            .execute(
                "UPDATE agent_sessions SET status = ?1
                 WHERE agent_id = ?2 AND ended_at IS NULL",
                params![status.to_string(), agent_id],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_agent_sessions(&self, parent_session_id: &str) -> Result<Vec<AgentSession>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, parent_session_id, agent_id, agent_type, status,
                        started_at, ended_at, edits, commands, task_id, transcript_path
                 FROM agent_sessions WHERE parent_session_id = ?1
                 ORDER BY started_at DESC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![parent_session_id], |row| {
                Ok(parse_agent_session_row(row))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn get_active_agent_sessions(&self) -> Result<Vec<AgentSession>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, parent_session_id, agent_id, agent_type, status,
                        started_at, ended_at, edits, commands, task_id, transcript_path
                 FROM agent_sessions WHERE ended_at IS NULL
                 ORDER BY started_at DESC",
            )
            .sq()?;
        let rows = stmt
            .query_map([], |row| Ok(parse_agent_session_row(row)))
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn increment_agent_edits(&self, agent_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE agent_sessions SET edits = edits + 1
                 WHERE agent_id = ?1 AND ended_at IS NULL",
                params![agent_id],
            )
            .sq()?;
        Ok(())
    }

    pub fn increment_agent_commands(&self, agent_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE agent_sessions SET commands = commands + 1
                 WHERE agent_id = ?1 AND ended_at IS NULL",
                params![agent_id],
            )
            .sq()?;
        Ok(())
    }

    pub fn update_agent_session_transcript_path(&self, agent_id: &str, path: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE agent_sessions SET transcript_path = ?1
                 WHERE agent_id = ?2 AND ended_at IS NULL",
                params![path, agent_id],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_agents_on_work_item(&self, work_item_id: &str) -> Result<Vec<AgentSession>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, parent_session_id, agent_id, agent_type, status,
                        started_at, ended_at, edits, commands, task_id, transcript_path
                 FROM agent_sessions WHERE task_id = ?1
                 ORDER BY started_at DESC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![work_item_id], |row| {
                Ok(parse_agent_session_row(row))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn update_agent_session_work_item(&self, agent_id: &str, work_item_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE agent_sessions SET task_id = ?1
                 WHERE agent_id = ?2 AND ended_at IS NULL",
                params![work_item_id, agent_id],
            )
            .sq()?;
        Ok(())
    }
}
