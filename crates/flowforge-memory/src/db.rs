use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use flowforge_core::{
    EditRecord, Error, LongTermPattern, Result, RoutingWeight, SessionInfo, ShortTermPattern,
};

pub struct MemoryDb {
    conn: Connection,
}

impl MemoryDb {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Sqlite(e.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|e| Error::Sqlite(e.to_string()))?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS key_value (
                key TEXT NOT NULL,
                value TEXT,
                namespace TEXT DEFAULT 'default',
                created_at TEXT,
                updated_at TEXT,
                PRIMARY KEY (key, namespace)
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                started_at TEXT,
                ended_at TEXT,
                cwd TEXT,
                edits INTEGER DEFAULT 0,
                commands INTEGER DEFAULT 0,
                summary TEXT
            );

            CREATE TABLE IF NOT EXISTS edits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                timestamp TEXT,
                file_path TEXT,
                operation TEXT,
                file_extension TEXT
            );

            CREATE TABLE IF NOT EXISTS patterns_short (
                id TEXT PRIMARY KEY,
                content TEXT,
                category TEXT,
                confidence REAL DEFAULT 0.5,
                usage_count INTEGER DEFAULT 0,
                created_at TEXT,
                last_used TEXT,
                embedding_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS patterns_long (
                id TEXT PRIMARY KEY,
                content TEXT,
                category TEXT,
                confidence REAL,
                usage_count INTEGER DEFAULT 0,
                success_count INTEGER DEFAULT 0,
                failure_count INTEGER DEFAULT 0,
                created_at TEXT,
                promoted_at TEXT,
                last_used TEXT,
                embedding_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS hnsw_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT,
                source_id TEXT,
                vector BLOB,
                created_at TEXT
            );

            CREATE TABLE IF NOT EXISTS routing_weights (
                task_pattern TEXT,
                agent_name TEXT,
                weight REAL DEFAULT 0.5,
                successes INTEGER DEFAULT 0,
                failures INTEGER DEFAULT 0,
                updated_at TEXT,
                PRIMARY KEY (task_pattern, agent_name)
            );
        ",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    // ── Key-Value ──

    pub fn kv_get(&self, key: &str, namespace: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM key_value WHERE key = ?1 AND namespace = ?2",
                params![key, namespace],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn kv_set(&self, key: &str, value: &str, namespace: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO key_value (key, value, namespace, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(key, namespace) DO UPDATE SET value = ?2, updated_at = ?4",
                params![key, value, namespace, now],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn kv_delete(&self, key: &str, namespace: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM key_value WHERE key = ?1 AND namespace = ?2",
                params![key, namespace],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn kv_list(&self, namespace: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM key_value WHERE namespace = ?1 ORDER BY key")
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![namespace], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn kv_search(&self, query: &str, limit: usize) -> Result<Vec<(String, String, String)>> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT key, value, namespace FROM key_value
                 WHERE key LIKE ?1 OR value LIKE ?1
                 ORDER BY updated_at DESC LIMIT ?2",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![pattern, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn count_kv(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM key_value", [], |row| row.get(0))
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    // ── Sessions ──

    pub fn create_session(&self, session: &SessionInfo) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO sessions (id, started_at, ended_at, cwd, edits, commands, summary)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    session.id,
                    session.started_at.to_rfc3339(),
                    session.ended_at.map(|t| t.to_rfc3339()),
                    session.cwd,
                    session.edits,
                    session.commands,
                    session.summary,
                ],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn end_session(&self, id: &str, ended_at: DateTime<Utc>) -> Result<()> {
        self.conn
            .execute(
                "UPDATE sessions SET ended_at = ?1 WHERE id = ?2",
                params![ended_at.to_rfc3339(), id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn get_current_session(&self) -> Result<Option<SessionInfo>> {
        self.conn
            .query_row(
                "SELECT id, started_at, ended_at, cwd, edits, commands, summary
                 FROM sessions WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(SessionInfo {
                        id: row.get(0)?,
                        started_at: parse_datetime(row.get::<_, String>(1)?),
                        ended_at: row.get::<_, Option<String>>(2)?.map(parse_datetime),
                        cwd: row.get(3)?,
                        edits: row.get(4)?,
                        commands: row.get(5)?,
                        summary: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionInfo>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, started_at, ended_at, cwd, edits, commands, summary
                 FROM sessions ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(SessionInfo {
                    id: row.get(0)?,
                    started_at: parse_datetime(row.get::<_, String>(1)?),
                    ended_at: row.get::<_, Option<String>>(2)?.map(parse_datetime),
                    cwd: row.get(3)?,
                    edits: row.get(4)?,
                    commands: row.get(5)?,
                    summary: row.get(6)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn increment_session_edits(&self, session_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE sessions SET edits = edits + 1 WHERE id = ?1",
                params![session_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn increment_session_commands(&self, session_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE sessions SET commands = commands + 1 WHERE id = ?1",
                params![session_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    // ── Edits ──

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
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn get_edits_for_session(&self, session_id: &str) -> Result<Vec<EditRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT session_id, timestamp, file_path, operation, file_extension
                 FROM edits WHERE session_id = ?1 ORDER BY timestamp",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
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
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn count_edits(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM edits", [], |row| row.get(0))
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    // ── Patterns (Short-term) ──

    pub fn store_pattern_short(&self, pattern: &ShortTermPattern) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO patterns_short
                 (id, content, category, confidence, usage_count, created_at, last_used, embedding_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    pattern.id,
                    pattern.content,
                    pattern.category,
                    pattern.confidence,
                    pattern.usage_count,
                    pattern.created_at.to_rfc3339(),
                    pattern.last_used.to_rfc3339(),
                    pattern.embedding_id,
                ],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn search_patterns_short(&self, query: &str, limit: usize) -> Result<Vec<ShortTermPattern>> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, content, category, confidence, usage_count, created_at, last_used, embedding_id
                 FROM patterns_short WHERE content LIKE ?1 OR category LIKE ?1
                 ORDER BY confidence DESC LIMIT ?2",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![pattern, limit], |row| {
                Ok(ShortTermPattern {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    category: row.get(2)?,
                    confidence: row.get(3)?,
                    usage_count: row.get(4)?,
                    created_at: parse_datetime(row.get::<_, String>(5)?),
                    last_used: parse_datetime(row.get::<_, String>(6)?),
                    embedding_id: row.get(7)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn get_all_patterns_short(&self) -> Result<Vec<ShortTermPattern>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, content, category, confidence, usage_count, created_at, last_used, embedding_id
                 FROM patterns_short ORDER BY last_used DESC",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ShortTermPattern {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    category: row.get(2)?,
                    confidence: row.get(3)?,
                    usage_count: row.get(4)?,
                    created_at: parse_datetime(row.get::<_, String>(5)?),
                    last_used: parse_datetime(row.get::<_, String>(6)?),
                    embedding_id: row.get(7)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn update_pattern_short_usage(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE patterns_short SET usage_count = usage_count + 1, last_used = ?1,
                 confidence = MIN(1.0, confidence + 0.05) WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn delete_pattern_short(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM patterns_short WHERE id = ?1", params![id])
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn count_patterns_short(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM patterns_short", [], |row| row.get(0))
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    // ── Patterns (Long-term) ──

    pub fn store_pattern_long(&self, pattern: &LongTermPattern) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO patterns_long
                 (id, content, category, confidence, usage_count, success_count, failure_count,
                  created_at, promoted_at, last_used, embedding_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    pattern.id,
                    pattern.content,
                    pattern.category,
                    pattern.confidence,
                    pattern.usage_count,
                    pattern.success_count,
                    pattern.failure_count,
                    pattern.created_at.to_rfc3339(),
                    pattern.promoted_at.to_rfc3339(),
                    pattern.last_used.to_rfc3339(),
                    pattern.embedding_id,
                ],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn search_patterns_long(&self, query: &str, limit: usize) -> Result<Vec<LongTermPattern>> {
        let pattern = format!("%{query}%");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, content, category, confidence, usage_count, success_count, failure_count,
                        created_at, promoted_at, last_used, embedding_id
                 FROM patterns_long WHERE content LIKE ?1 OR category LIKE ?1
                 ORDER BY confidence DESC LIMIT ?2",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![pattern, limit], |row| {
                Ok(LongTermPattern {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    category: row.get(2)?,
                    confidence: row.get(3)?,
                    usage_count: row.get(4)?,
                    success_count: row.get(5)?,
                    failure_count: row.get(6)?,
                    created_at: parse_datetime(row.get::<_, String>(7)?),
                    promoted_at: parse_datetime(row.get::<_, String>(8)?),
                    last_used: parse_datetime(row.get::<_, String>(9)?),
                    embedding_id: row.get(10)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn count_patterns_long(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM patterns_long", [], |row| row.get(0))
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn count_patterns(&self) -> Result<u64> {
        let short = self.count_patterns_short()?;
        let long = self.count_patterns_long()?;
        Ok(short + long)
    }

    pub fn get_top_patterns(&self, limit: usize) -> Result<Vec<ShortTermPattern>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, content, category, confidence, usage_count, created_at, last_used, embedding_id
                 FROM patterns_short ORDER BY confidence DESC, usage_count DESC LIMIT ?1",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(ShortTermPattern {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    category: row.get(2)?,
                    confidence: row.get(3)?,
                    usage_count: row.get(4)?,
                    created_at: parse_datetime(row.get::<_, String>(5)?),
                    last_used: parse_datetime(row.get::<_, String>(6)?),
                    embedding_id: row.get(7)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    // ── Routing Weights ──

    pub fn get_routing_weight(&self, task_pattern: &str, agent_name: &str) -> Result<Option<RoutingWeight>> {
        self.conn
            .query_row(
                "SELECT task_pattern, agent_name, weight, successes, failures, updated_at
                 FROM routing_weights WHERE task_pattern = ?1 AND agent_name = ?2",
                params![task_pattern, agent_name],
                |row| {
                    Ok(RoutingWeight {
                        task_pattern: row.get(0)?,
                        agent_name: row.get(1)?,
                        weight: row.get(2)?,
                        successes: row.get(3)?,
                        failures: row.get(4)?,
                        updated_at: parse_datetime(row.get::<_, String>(5)?),
                    })
                },
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn get_all_routing_weights(&self) -> Result<Vec<RoutingWeight>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT task_pattern, agent_name, weight, successes, failures, updated_at
                 FROM routing_weights ORDER BY weight DESC",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(RoutingWeight {
                    task_pattern: row.get(0)?,
                    agent_name: row.get(1)?,
                    weight: row.get(2)?,
                    successes: row.get(3)?,
                    failures: row.get(4)?,
                    updated_at: parse_datetime(row.get::<_, String>(5)?),
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn record_routing_success(&self, task_pattern: &str, agent_name: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO routing_weights (task_pattern, agent_name, weight, successes, failures, updated_at)
                 VALUES (?1, ?2, 0.6, 1, 0, ?3)
                 ON CONFLICT(task_pattern, agent_name) DO UPDATE SET
                   successes = successes + 1,
                   weight = MIN(1.0, weight + 0.05),
                   updated_at = ?3",
                params![task_pattern, agent_name, now],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn record_routing_failure(&self, task_pattern: &str, agent_name: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO routing_weights (task_pattern, agent_name, weight, successes, failures, updated_at)
                 VALUES (?1, ?2, 0.4, 0, 1, ?3)
                 ON CONFLICT(task_pattern, agent_name) DO UPDATE SET
                   failures = failures + 1,
                   weight = MAX(0.0, weight - 0.05),
                   updated_at = ?3",
                params![task_pattern, agent_name, now],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn count_routing_weights(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM routing_weights", [], |row| row.get(0))
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    // ── HNSW Entries ──

    pub fn store_vector(&self, source_type: &str, source_id: &str, vector: &[f32]) -> Result<i64> {
        let blob = vector_to_blob(vector);
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO hnsw_entries (source_type, source_id, vector, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![source_type, source_id, blob, now],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_all_vectors(&self) -> Result<Vec<(i64, String, String, Vec<f32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, source_type, source_id, vector FROM hnsw_entries")
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let blob: Vec<u8> = row.get(3)?;
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    blob_to_vector(&blob),
                ))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn delete_vectors_for_source(&self, source_type: &str, source_id: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM hnsw_entries WHERE source_type = ?1 AND source_id = ?2",
                params![source_type, source_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }
}

fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_vector(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
