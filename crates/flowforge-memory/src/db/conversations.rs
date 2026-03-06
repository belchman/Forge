use rusqlite::params;

use flowforge_core::{ConversationMessage, Result};

use super::row_parsers::parse_conversation_message_row;
use super::{MemoryDb, SqliteExt};

impl MemoryDb {
    // ── Conversation Messages ──

    pub fn store_conversation_message(&self, msg: &ConversationMessage) -> Result<i64> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO conversation_messages
                 (session_id, message_index, message_type, role, content, model,
                  message_id, parent_uuid, timestamp, metadata, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    msg.session_id,
                    msg.message_index,
                    msg.message_type,
                    msg.role,
                    msg.content,
                    msg.model,
                    msg.message_id,
                    msg.parent_uuid,
                    msg.timestamp.to_rfc3339(),
                    msg.metadata,
                    msg.source,
                ],
            )
            .sq()?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn ingest_transcript(&self, session_id: &str, transcript_path: &str) -> Result<u32> {
        let latest = self.get_latest_message_index(session_id)?;
        let messages = flowforge_core::transcript::parse_transcript(transcript_path, session_id)?;

        let mut count = 0u32;
        for msg in &messages {
            if msg.message_index >= latest {
                self.store_conversation_message(msg)?;
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn get_conversation_messages(
        &self,
        session_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ConversationMessage>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, message_index, message_type, role, content,
                        model, message_id, parent_uuid, timestamp, metadata, source
                 FROM conversation_messages WHERE session_id = ?1
                 ORDER BY message_index ASC LIMIT ?2 OFFSET ?3",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id, limit, offset], |row| {
                Ok(parse_conversation_message_row(row))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn get_conversation_message_count(&self, session_id: &str) -> Result<u32> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_messages WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .sq()
    }

    pub fn get_conversation_messages_range(
        &self,
        session_id: &str,
        from: u32,
        to: u32,
    ) -> Result<Vec<ConversationMessage>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, message_index, message_type, role, content,
                        model, message_id, parent_uuid, timestamp, metadata, source
                 FROM conversation_messages
                 WHERE session_id = ?1 AND message_index >= ?2 AND message_index <= ?3
                 ORDER BY message_index ASC",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id, from, to], |row| {
                Ok(parse_conversation_message_row(row))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn get_latest_message_index(&self, session_id: &str) -> Result<u32> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(message_index) + 1, 0) FROM conversation_messages WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .sq()
    }

    /// Search conversation messages using semantic vector search.
    pub fn search_conversation_messages_semantic(
        &self,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<ConversationMessage>> {
        let results = self.search_vectors(query_vec, &["conversation"], k)?;

        let mut messages = Vec::new();
        for result in &results {
            if result.similarity < 0.3 {
                continue;
            }
            // Parse source_id: "session_id:message_index"
            let parts: Vec<&str> = result.source_id.splitn(2, ':').collect();
            if parts.len() == 2 {
                let session_id = parts[0];
                if let Ok(msg_idx) = parts[1].parse::<u32>() {
                    let msgs = self.get_conversation_messages_range(session_id, msg_idx, msg_idx)?;
                    if let Some(msg) = msgs.into_iter().next() {
                        messages.push(msg);
                    }
                }
            }
        }
        Ok(messages)
    }

    pub fn search_conversation_messages(
        &self,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationMessage>> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, message_index, message_type, role, content,
                        model, message_id, parent_uuid, timestamp, metadata, source
                 FROM conversation_messages
                 WHERE session_id = ?1 AND content LIKE ?2
                 ORDER BY message_index ASC LIMIT ?3",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id, pattern, limit], |row| {
                Ok(parse_conversation_message_row(row))
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }
}
