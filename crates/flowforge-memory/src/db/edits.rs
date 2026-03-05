use rusqlite::params;

use flowforge_core::{EditRecord, Result};

use super::{parse_datetime, MemoryDb, SqliteExt};

impl MemoryDb {
    pub fn record_edit(&self, edit: &EditRecord) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO edits (session_id, timestamp, file_path, operation, file_extension)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    edit.session_id,
                    edit.timestamp.to_rfc3339(),
                    edit.file_path,
                    edit.operation,
                    edit.file_extension,
                ],
            )
            .sq()?;
        Ok(())
    }

    pub fn get_edits_for_session(&self, session_id: &str) -> Result<Vec<EditRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT session_id, timestamp, file_path, operation, file_extension
                 FROM edits WHERE session_id = ?1 ORDER BY timestamp",
            )
            .sq()?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(EditRecord {
                    session_id: row.get(0)?,
                    timestamp: parse_datetime(row.get::<_, String>(1)?),
                    file_path: row.get(2)?,
                    operation: row.get(3)?,
                    file_extension: row.get(4)?,
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn count_edits(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM edits", [], |row| row.get(0))
            .sq()
    }
}
