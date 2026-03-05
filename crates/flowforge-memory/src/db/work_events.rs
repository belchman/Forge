use chrono::{DateTime, Utc};
use rusqlite::params;

use flowforge_core::{Result, WorkEvent};

use super::{parse_datetime, MemoryDb, SqliteExt};

impl MemoryDb {
    // ── Work Events ──

    pub fn record_work_event(&self, event: &WorkEvent) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO work_events (work_item_id, event_type, old_value, new_value, actor, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    event.work_item_id,
                    event.event_type,
                    event.old_value,
                    event.new_value,
                    event.actor,
                    event.timestamp.to_rfc3339(),
                ],
            )
            .sq()?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_work_events(&self, work_item_id: &str, limit: usize) -> Result<Vec<WorkEvent>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, work_item_id, event_type, old_value, new_value, actor, timestamp
                 FROM work_events WHERE work_item_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![work_item_id, limit], |row| {
                Ok(WorkEvent {
                    id: row.get(0)?,
                    work_item_id: row.get(1)?,
                    event_type: row.get(2)?,
                    old_value: row.get(3)?,
                    new_value: row.get(4)?,
                    actor: row.get(5)?,
                    timestamp: parse_datetime(row.get::<_, String>(6)?),
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn get_recent_work_events(&self, limit: usize) -> Result<Vec<WorkEvent>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, work_item_id, event_type, old_value, new_value, actor, timestamp
                 FROM work_events ORDER BY timestamp DESC LIMIT ?1",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(WorkEvent {
                    id: row.get(0)?,
                    work_item_id: row.get(1)?,
                    event_type: row.get(2)?,
                    old_value: row.get(3)?,
                    new_value: row.get(4)?,
                    actor: row.get(5)?,
                    timestamp: parse_datetime(row.get::<_, String>(6)?),
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn get_recent_work_events_since(
        &self,
        since: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<WorkEvent>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, work_item_id, event_type, old_value, new_value, actor, timestamp
                 FROM work_events WHERE timestamp >= ?1 ORDER BY timestamp DESC LIMIT ?2",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![since.to_rfc3339(), limit], |row| {
                Ok(WorkEvent {
                    id: row.get(0)?,
                    work_item_id: row.get(1)?,
                    event_type: row.get(2)?,
                    old_value: row.get(3)?,
                    new_value: row.get(4)?,
                    actor: row.get(5)?,
                    timestamp: parse_datetime(row.get::<_, String>(6)?),
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }
}
