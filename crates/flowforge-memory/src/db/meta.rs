use rusqlite::{params, OptionalExtension};

use flowforge_core::Result;

use super::{MemoryDb, SqliteExt};

impl MemoryDb {
    // ── Meta Key-Value ──

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM flowforge_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .sq()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO flowforge_meta (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .sq()?;
        Ok(())
    }
}
