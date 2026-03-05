use rusqlite::params;

use flowforge_core::{Checkpoint, Result, SessionFork};

use super::row_parsers::{parse_checkpoint_row, parse_session_fork_row};
use super::{MemoryDb, SqliteExt};

use rusqlite::OptionalExtension;

impl MemoryDb {
    // ── Checkpoints ──

    pub fn create_checkpoint(&self, cp: &Checkpoint) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO checkpoints (id, session_id, name, message_index, description, git_ref, created_at, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    cp.id,
                    cp.session_id,
                    cp.name,
                    cp.message_index,
                    cp.description,
                    cp.git_ref,
                    cp.created_at.to_rfc3339(),
                    cp.metadata,
                ],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_checkpoint(&self, id: &str) -> Result<Option<Checkpoint>> {
        self.conn
            .query_row(
                "SELECT id, session_id, name, message_index, description, git_ref, created_at, metadata
                 FROM checkpoints WHERE id = ?1",
                params![id],
                |row| Ok(parse_checkpoint_row(row)),
            )
            .optional()
            .sq()
    }

    pub fn get_checkpoint_by_name(
        &self,
        session_id: &str,
        name: &str,
    ) -> Result<Option<Checkpoint>> {
        self.conn
            .query_row(
                "SELECT id, session_id, name, message_index, description, git_ref, created_at, metadata
                 FROM checkpoints WHERE session_id = ?1 AND name = ?2",
                params![session_id, name],
                |row| Ok(parse_checkpoint_row(row)),
            )
            .optional()
            .sq()
    }

    pub fn list_checkpoints(&self, session_id: &str) -> Result<Vec<Checkpoint>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, name, message_index, description, git_ref, created_at, metadata
                 FROM checkpoints WHERE session_id = ?1 ORDER BY message_index ASC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id], |row| Ok(parse_checkpoint_row(row)))
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn delete_checkpoint(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM checkpoints WHERE id = ?1", params![id])
            .sq()?;
        Ok(())
    }

    // ── Session Forks ──

    pub fn fork_conversation(
        &self,
        source_id: &str,
        target_id: &str,
        up_to_index: u32,
    ) -> Result<u32> {
        let count: u32 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_messages
                 WHERE session_id = ?1 AND message_index <= ?2",
                params![source_id, up_to_index],
                |row| row.get(0),
            )
            .sq()?;

        self.conn
            .execute(
                "INSERT OR IGNORE INTO conversation_messages
                 (session_id, message_index, message_type, role, content, model,
                  message_id, parent_uuid, timestamp, metadata, source)
                 SELECT ?1, message_index, message_type, role, content, model,
                        message_id, parent_uuid, timestamp, metadata, 'forked'
                 FROM conversation_messages
                 WHERE session_id = ?2 AND message_index <= ?3",
                params![target_id, source_id, up_to_index],
            )
            .sq()?;

        Ok(count)
    }

    pub fn create_session_fork(&self, fork: &SessionFork) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO session_forks
                 (id, source_session_id, target_session_id, fork_message_index,
                  checkpoint_id, reason, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    fork.id,
                    fork.source_session_id,
                    fork.target_session_id,
                    fork.fork_message_index,
                    fork.checkpoint_id,
                    fork.reason,
                    fork.created_at.to_rfc3339(),
                ],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_session_forks(&self, session_id: &str) -> Result<Vec<SessionFork>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, source_session_id, target_session_id, fork_message_index,
                        checkpoint_id, reason, created_at
                 FROM session_forks
                 WHERE source_session_id = ?1 OR target_session_id = ?1
                 ORDER BY created_at DESC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id], |row| Ok(parse_session_fork_row(row)))
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn get_session_lineage(&self, session_id: &str) -> Result<Vec<SessionFork>> {
        // Trace fork chain to root: follow source_session_id backwards
        let mut lineage = Vec::new();
        let mut current = session_id.to_string();
        for _ in 0..50 {
            // safety limit
            let fork: Option<SessionFork> = self
                .conn
                .query_row(
                    "SELECT id, source_session_id, target_session_id, fork_message_index,
                            checkpoint_id, reason, created_at
                     FROM session_forks WHERE target_session_id = ?1",
                    params![current],
                    |row| Ok(parse_session_fork_row(row)),
                )
                .optional()
                .sq()?;
            match fork {
                Some(f) => {
                    current = f.source_session_id.clone();
                    lineage.push(f);
                }
                None => break,
            }
        }
        lineage.reverse();
        Ok(lineage)
    }
}
