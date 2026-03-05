use chrono::Utc;
use rusqlite::params;

use flowforge_core::{MailboxMessage, Result};

use super::row_parsers::parse_mailbox_message_row;
use super::{MemoryDb, SqliteExt};

impl MemoryDb {
    // ── Agent Mailbox ──

    pub fn send_mailbox_message(&self, msg: &MailboxMessage) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO agent_mailbox
                 (work_item_id, from_session_id, from_agent_name, to_session_id, to_agent_name,
                  message_type, content, priority, created_at, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    msg.work_item_id,
                    msg.from_session_id,
                    msg.from_agent_name,
                    msg.to_session_id,
                    msg.to_agent_name,
                    msg.message_type,
                    msg.content,
                    msg.priority,
                    msg.created_at.to_rfc3339(),
                    msg.metadata,
                ],
            )
            .sq()?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_unread_messages(&self, session_id: &str) -> Result<Vec<MailboxMessage>> {
        // Get messages targeted at this session OR broadcasts (to_session_id IS NULL)
        // for work items this agent is on
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, work_item_id, from_session_id, from_agent_name, to_session_id,
                        to_agent_name, message_type, content, priority, read_at, created_at, metadata
                 FROM agent_mailbox
                 WHERE read_at IS NULL
                   AND (to_session_id = ?1 OR (to_session_id IS NULL AND from_session_id != ?1))
                 ORDER BY priority ASC, created_at ASC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(parse_mailbox_message_row(row))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn mark_messages_read(&self, session_id: &str) -> Result<u32> {
        let now = Utc::now().to_rfc3339();
        let count = self
            .conn
            .execute(
                "UPDATE agent_mailbox SET read_at = ?1
                 WHERE read_at IS NULL
                   AND (to_session_id = ?2 OR (to_session_id IS NULL AND from_session_id != ?2))",
                params![now, session_id],
            )
            .sq()?;
        Ok(count as u32)
    }

    pub fn mark_message_read(&self, id: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE agent_mailbox SET read_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_mailbox_history(
        &self,
        work_item_id: &str,
        limit: usize,
    ) -> Result<Vec<MailboxMessage>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, work_item_id, from_session_id, from_agent_name, to_session_id,
                        to_agent_name, message_type, content, priority, read_at, created_at, metadata
                 FROM agent_mailbox WHERE work_item_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![work_item_id, limit], |row| {
                Ok(parse_mailbox_message_row(row))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }
}
