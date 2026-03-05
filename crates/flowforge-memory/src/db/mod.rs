mod agent_sessions;
mod checkpoints;
mod conversations;
mod edits;
mod effectiveness;
mod guidance;
mod kv;
mod mailbox;
mod patterns;
mod routing;
mod row_parsers;
mod sessions;
mod trajectories;
mod vectors;
mod work_events;
mod work_items;

pub use effectiveness::PatternEffectiveness;

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use flowforge_core::{Error, Result};

/// Extension trait to eliminate repetitive `.map_err(|e| Error::Sqlite(e.to_string()))` calls.
pub(crate) trait SqliteExt<T> {
    fn sq(self) -> Result<T>;
}

impl<T> SqliteExt<T> for std::result::Result<T, rusqlite::Error> {
    fn sq(self) -> Result<T> {
        self.map_err(|e| Error::Sqlite(e.to_string()))
    }
}

/// (db_id, source_type, source_id, vector)
pub(crate) type VectorEntry = (i64, String, String, Vec<f32>);

/// Bump this whenever init_schema() changes (new tables, columns, indexes).
const SCHEMA_VERSION: u32 = 3;

pub struct MemoryDb {
    pub(crate) conn: Connection,
}

impl MemoryDb {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Sqlite(e.to_string()))?;
        }
        let conn = Connection::open(path).sq()?;

        // WAL mode + relaxed sync for better write throughput
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA wal_autocheckpoint=100;",
        )
        .sq()?;

        let db = Self { conn };

        // Skip full DDL if schema is already at the current version
        let stored_version: Option<u32> = db
            .conn
            .query_row(
                "SELECT value FROM flowforge_meta WHERE key = 'schema_version'",
                [],
                |row| {
                    let s: String = row.get(0)?;
                    Ok(s.parse::<u32>().unwrap_or(0))
                },
            )
            .optional()
            .unwrap_or(None);

        if stored_version != Some(SCHEMA_VERSION) {
            db.init_schema()?;
            // Stamp version after successful init
            db.conn
                .execute(
                    "INSERT OR REPLACE INTO flowforge_meta (key, value) VALUES ('schema_version', ?1)",
                    params![SCHEMA_VERSION.to_string()],
                )
                .sq()?;
        } else {
            // Still need foreign keys even when skipping DDL
            db.conn.execute_batch("PRAGMA foreign_keys = ON").sq()?;
        }

        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        // Enable foreign key enforcement (SQLite doesn't enable by default)
        self.conn.execute_batch("PRAGMA foreign_keys = ON").sq()?;

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

            CREATE TABLE IF NOT EXISTS flowforge_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
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

            CREATE TABLE IF NOT EXISTS work_items (
                id TEXT PRIMARY KEY,
                external_id TEXT,
                backend TEXT NOT NULL,
                item_type TEXT DEFAULT 'task',
                title TEXT NOT NULL,
                description TEXT,
                status TEXT DEFAULT 'pending',
                assignee TEXT,
                parent_id TEXT,
                priority INTEGER DEFAULT 2,
                labels TEXT,
                created_at TEXT,
                updated_at TEXT,
                completed_at TEXT,
                session_id TEXT,
                metadata TEXT
            );

            CREATE TABLE IF NOT EXISTS work_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                work_item_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                old_value TEXT,
                new_value TEXT,
                actor TEXT,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (work_item_id) REFERENCES work_items(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS work_tracking_config (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_work_items_status ON work_items(status);
            CREATE INDEX IF NOT EXISTS idx_work_items_backend ON work_items(backend);
            CREATE INDEX IF NOT EXISTS idx_work_items_parent ON work_items(parent_id);
            CREATE INDEX IF NOT EXISTS idx_work_events_item ON work_events(work_item_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_work_items_external_id
                ON work_items(external_id) WHERE external_id IS NOT NULL;

            CREATE TABLE IF NOT EXISTS agent_sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                agent_type TEXT NOT NULL DEFAULT 'general',
                status TEXT NOT NULL DEFAULT 'active',
                started_at TEXT NOT NULL,
                ended_at TEXT,
                edits INTEGER DEFAULT 0,
                commands INTEGER DEFAULT 0,
                task_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_parent ON agent_sessions(parent_session_id);
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_agent ON agent_sessions(agent_id);

            CREATE TABLE IF NOT EXISTS conversation_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                message_index INTEGER NOT NULL,
                message_type TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                model TEXT,
                message_id TEXT,
                parent_uuid TEXT,
                timestamp TEXT NOT NULL,
                metadata TEXT,
                source TEXT DEFAULT 'transcript',
                UNIQUE(session_id, message_index)
            );
            CREATE INDEX IF NOT EXISTS idx_conv_session ON conversation_messages(session_id);
            CREATE INDEX IF NOT EXISTS idx_conv_type ON conversation_messages(message_type);
            CREATE INDEX IF NOT EXISTS idx_conv_timestamp ON conversation_messages(timestamp);

            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                name TEXT NOT NULL,
                message_index INTEGER NOT NULL,
                description TEXT,
                git_ref TEXT,
                created_at TEXT NOT NULL,
                metadata TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_checkpoint_name ON checkpoints(session_id, name);

            CREATE TABLE IF NOT EXISTS session_forks (
                id TEXT PRIMARY KEY,
                source_session_id TEXT NOT NULL,
                target_session_id TEXT NOT NULL,
                fork_message_index INTEGER NOT NULL,
                checkpoint_id TEXT,
                reason TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_forks_source ON session_forks(source_session_id);
            CREATE INDEX IF NOT EXISTS idx_forks_target ON session_forks(target_session_id);

            CREATE TABLE IF NOT EXISTS agent_mailbox (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                work_item_id TEXT NOT NULL,
                from_session_id TEXT NOT NULL,
                from_agent_name TEXT NOT NULL,
                to_session_id TEXT,
                to_agent_name TEXT,
                message_type TEXT DEFAULT 'text',
                content TEXT NOT NULL,
                priority INTEGER DEFAULT 2,
                read_at TEXT,
                created_at TEXT NOT NULL,
                metadata TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_mailbox_work ON agent_mailbox(work_item_id);
            CREATE INDEX IF NOT EXISTS idx_mailbox_to ON agent_mailbox(to_session_id);
            CREATE INDEX IF NOT EXISTS idx_mailbox_unread ON agent_mailbox(to_session_id, read_at);
            CREATE INDEX IF NOT EXISTS idx_mailbox_from ON agent_mailbox(from_session_id);

            CREATE TABLE IF NOT EXISTS trust_scores (
                session_id TEXT PRIMARY KEY,
                score REAL DEFAULT 0.5,
                total_checks INTEGER DEFAULT 0,
                denials INTEGER DEFAULT 0,
                asks INTEGER DEFAULT 0,
                allows INTEGER DEFAULT 0,
                last_updated TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS gate_decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                rule_id TEXT,
                gate_name TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                action TEXT NOT NULL,
                reason TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                trust_before REAL,
                trust_after REAL,
                timestamp TEXT NOT NULL,
                hash TEXT NOT NULL,
                prev_hash TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS trajectories (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                work_item_id TEXT,
                agent_name TEXT,
                task_description TEXT,
                status TEXT DEFAULT 'recording',
                started_at TEXT NOT NULL,
                ended_at TEXT,
                verdict TEXT,
                confidence REAL,
                metadata TEXT,
                embedding_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS trajectory_steps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                trajectory_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                tool_name TEXT NOT NULL,
                tool_input_hash TEXT,
                outcome TEXT DEFAULT 'success',
                duration_ms INTEGER,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (trajectory_id) REFERENCES trajectories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_trajectory_steps_traj ON trajectory_steps(trajectory_id);
            CREATE INDEX IF NOT EXISTS idx_trajectories_session ON trajectories(session_id);
            CREATE INDEX IF NOT EXISTS idx_trajectories_status ON trajectories(status);

            CREATE TABLE IF NOT EXISTS pattern_clusters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                centroid BLOB NOT NULL,
                member_count INTEGER NOT NULL DEFAULT 0,
                p95_distance REAL NOT NULL DEFAULT 0.0,
                avg_confidence REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL,
                last_recomputed TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS context_injections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                trajectory_id TEXT,
                injection_type TEXT NOT NULL,
                reference_id TEXT,
                similarity REAL,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_ctx_inject_session ON context_injections(session_id);
            CREATE INDEX IF NOT EXISTS idx_ctx_inject_trajectory ON context_injections(trajectory_id);
        ",
            )
            .sq()?;

        // Migrations: add columns to existing tables if missing
        self.migrate_add_column("sessions", "transcript_path", "TEXT")?;
        self.migrate_add_column("agent_sessions", "transcript_path", "TEXT")?;

        // Work-stealing migrations
        self.migrate_add_column("work_items", "claimed_by", "TEXT")?;
        self.migrate_add_column("work_items", "claimed_at", "TEXT")?;
        self.migrate_add_column("work_items", "last_heartbeat", "TEXT")?;
        self.migrate_add_column("work_items", "progress", "INTEGER DEFAULT 0")?;
        self.migrate_add_column("work_items", "stealable", "INTEGER DEFAULT 0")?;

        // Work-stealing indexes (best-effort)
        let _ = self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_work_items_claimed ON work_items(claimed_by);
             CREATE INDEX IF NOT EXISTS idx_work_items_stealable ON work_items(stealable);
             CREATE INDEX IF NOT EXISTS idx_work_items_heartbeat ON work_items(last_heartbeat);",
        );

        // Clustering migrations
        self.migrate_add_column(
            "hnsw_entries",
            "cluster_id",
            "INTEGER REFERENCES pattern_clusters(id)",
        )?;
        let _ = self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_hnsw_entries_cluster ON hnsw_entries(cluster_id);",
        );

        // Effectiveness tracking migration
        self.migrate_add_column("context_injections", "effectiveness", "TEXT")?;

        // Pattern effectiveness table + columns
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS pattern_effectiveness (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    pattern_id TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    outcome TEXT NOT NULL,
                    similarity REAL DEFAULT 0.0,
                    timestamp TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_pattern_eff_pattern ON pattern_effectiveness(pattern_id);
                CREATE INDEX IF NOT EXISTS idx_pattern_eff_session ON pattern_effectiveness(session_id);",
            )
            .sq()?;
        self.migrate_add_column("patterns_long", "effectiveness_score", "REAL DEFAULT 0.0")?;
        self.migrate_add_column(
            "patterns_long",
            "effectiveness_samples",
            "INTEGER DEFAULT 0",
        )?;
        self.migrate_add_column("patterns_short", "effectiveness_score", "REAL DEFAULT 0.0")?;
        self.migrate_add_column(
            "patterns_short",
            "effectiveness_samples",
            "INTEGER DEFAULT 0",
        )?;

        // Anti-thrashing columns for work-stealing
        self.migrate_add_column("work_items", "steal_count", "INTEGER DEFAULT 0")?;
        self.migrate_add_column("work_items", "last_stolen_at", "TEXT")?;

        Ok(())
    }

    fn migrate_add_column(&self, table: &str, column: &str, col_type: &str) -> Result<()> {
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {col_type}");
        match self.conn.execute_batch(&sql) {
            Ok(()) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("duplicate column name") || msg.contains("already exists") {
                    Ok(()) // column already present
                } else {
                    Err(Error::Sqlite(msg))
                }
            }
        }
    }

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

pub(crate) fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

pub(crate) fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub(crate) fn blob_to_vector(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowforge_core::{
        types::{GateAction, GateDecision, RiskLevel},
        SessionInfo, WorkEvent, WorkFilter, WorkItem,
    };

    fn test_db() -> MemoryDb {
        MemoryDb::open(Path::new(":memory:")).unwrap()
    }

    fn test_work_item(id: &str, title: &str) -> WorkItem {
        WorkItem {
            id: id.to_string(),
            external_id: None,
            backend: "flowforge".to_string(),
            item_type: "task".to_string(),
            title: title.to_string(),
            description: None,
            status: "pending".to_string(),
            assignee: None,
            parent_id: None,
            priority: 2,
            labels: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            session_id: None,
            metadata: None,
            claimed_by: None,
            claimed_at: None,
            last_heartbeat: None,
            progress: 0,
            stealable: false,
        }
    }

    #[test]
    fn test_work_item_crud() {
        let db = test_db();
        let item = test_work_item("wi-1", "Fix login bug");
        db.create_work_item(&item).unwrap();
        let fetched = db.get_work_item("wi-1").unwrap().unwrap();
        assert_eq!(fetched.title, "Fix login bug");
        assert_eq!(fetched.status, "pending");
        db.update_work_item_status("wi-1", "in_progress").unwrap();
        let updated = db.get_work_item("wi-1").unwrap().unwrap();
        assert_eq!(updated.status, "in_progress");
        db.update_work_item_assignee("wi-1", "agent:coder").unwrap();
        let assigned = db.get_work_item("wi-1").unwrap().unwrap();
        assert_eq!(assigned.assignee, Some("agent:coder".to_string()));
        db.update_work_item_status("wi-1", "completed").unwrap();
        let completed = db.get_work_item("wi-1").unwrap().unwrap();
        assert_eq!(completed.status, "completed");
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_work_item_external_id_lookup() {
        let db = test_db();
        let mut item = test_work_item("wi-2", "External task");
        item.external_id = Some("kbs-123".to_string());
        item.backend = "kanbus".to_string();
        db.create_work_item(&item).unwrap();
        let fetched = db.get_work_item_by_external_id("kbs-123").unwrap().unwrap();
        assert_eq!(fetched.id, "wi-2");
        assert_eq!(fetched.backend, "kanbus");
    }

    #[test]
    fn test_work_item_unique_external_id() {
        let db = test_db();
        let mut item1 = test_work_item("wi-3", "First");
        item1.external_id = Some("ext-dup".to_string());
        db.create_work_item(&item1).unwrap();
        let mut item2 = test_work_item("wi-4", "Second");
        item2.external_id = Some("ext-dup".to_string());
        let result = db.create_work_item(&item2);
        if result.is_ok() {
            let found = db.get_work_item_by_external_id("ext-dup").unwrap().unwrap();
            assert_eq!(found.title, "Second");
        }
    }

    #[test]
    fn test_work_item_list_filter() {
        let db = test_db();
        db.create_work_item(&test_work_item("wi-a", "Task A"))
            .unwrap();
        db.create_work_item(&test_work_item("wi-b", "Task B"))
            .unwrap();
        db.update_work_item_status("wi-b", "completed").unwrap();
        let all = db.list_work_items(&WorkFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
        let pending = db
            .list_work_items(&WorkFilter {
                status: Some("pending".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "wi-a");
        let count = db.count_work_items_by_status("completed").unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_work_item_delete_cascades_events() {
        let db = test_db();
        db.create_work_item(&test_work_item("wi-del", "To delete"))
            .unwrap();
        let event = WorkEvent {
            id: 0,
            work_item_id: "wi-del".to_string(),
            event_type: "created".to_string(),
            old_value: None,
            new_value: Some("To delete".to_string()),
            actor: Some("test".to_string()),
            timestamp: Utc::now(),
        };
        db.record_work_event(&event).unwrap();
        assert_eq!(db.get_work_events("wi-del", 10).unwrap().len(), 1);
        db.delete_work_item("wi-del").unwrap();
        assert!(db.get_work_item("wi-del").unwrap().is_none());
        assert_eq!(db.get_work_events("wi-del", 10).unwrap().len(), 0);
    }

    #[test]
    fn test_work_events() {
        let db = test_db();
        db.create_work_item(&test_work_item("wi-ev", "Event test"))
            .unwrap();
        let event1 = WorkEvent {
            id: 0,
            work_item_id: "wi-ev".to_string(),
            event_type: "created".to_string(),
            old_value: None,
            new_value: Some("Event test".to_string()),
            actor: Some("user".to_string()),
            timestamp: Utc::now(),
        };
        let event2 = WorkEvent {
            id: 0,
            work_item_id: "wi-ev".to_string(),
            event_type: "status_changed".to_string(),
            old_value: Some("pending".to_string()),
            new_value: Some("in_progress".to_string()),
            actor: Some("agent:coder".to_string()),
            timestamp: Utc::now(),
        };
        db.record_work_event(&event1).unwrap();
        db.record_work_event(&event2).unwrap();
        assert_eq!(db.get_work_events("wi-ev", 10).unwrap().len(), 2);
        let recent = db.get_recent_work_events(1).unwrap();
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_work_item_backend_update() {
        let db = test_db();
        db.create_work_item(&test_work_item("wi-push", "Push test"))
            .unwrap();
        assert_eq!(
            db.get_work_item("wi-push").unwrap().unwrap().backend,
            "flowforge"
        );
        db.update_work_item_backend("wi-push", "kanbus").unwrap();
        assert_eq!(
            db.get_work_item("wi-push").unwrap().unwrap().backend,
            "kanbus"
        );
    }

    #[test]
    fn test_session_lifecycle() {
        let db = test_db();
        let session = SessionInfo {
            id: "sess-1".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            cwd: "/tmp".to_string(),
            edits: 0,
            commands: 0,
            summary: None,
            transcript_path: None,
        };
        db.create_session(&session).unwrap();
        assert_eq!(db.get_current_session().unwrap().unwrap().id, "sess-1");
        db.increment_session_edits("sess-1").unwrap();
        db.increment_session_commands("sess-1").unwrap();
        let updated = db.get_current_session().unwrap().unwrap();
        assert_eq!(updated.edits, 1);
        assert_eq!(updated.commands, 1);
        db.end_session("sess-1", Utc::now()).unwrap();
        assert!(db.get_current_session().unwrap().is_none());
        let sessions = db.list_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].ended_at.is_some());
    }

    #[test]
    fn test_kv_operations() {
        let db = test_db();
        db.kv_set("test-key", "test-value", "default").unwrap();
        assert_eq!(
            db.kv_get("test-key", "default").unwrap(),
            Some("test-value".to_string())
        );
        assert!(db.kv_get("missing", "default").unwrap().is_none());
        db.kv_delete("test-key", "default").unwrap();
        assert!(db.kv_get("test-key", "default").unwrap().is_none());
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let db = test_db();
        let fk_status: i32 = db
            .conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_status, 1);
    }

    #[test]
    fn test_gate_decisions_asc_order() {
        use sha2::{Digest, Sha256};
        let db = test_db();
        let session_id = "test-session";
        let mut prev_hash = String::new();
        let tools = ["Bash", "Read", "Edit"];
        for (i, tool) in tools.iter().enumerate() {
            let reason = format!("reason-{}", i);
            let input = format!("{}{}{}{}", session_id, tool, reason, prev_hash);
            let hash = format!("{:x}", Sha256::digest(input.as_bytes()));
            let decision = GateDecision {
                id: 0,
                session_id: session_id.to_string(),
                rule_id: Some(format!("rule-{}", i)),
                gate_name: "test_gate".to_string(),
                tool_name: tool.to_string(),
                action: GateAction::Allow,
                reason,
                risk_level: RiskLevel::Low,
                trust_before: 1.0,
                trust_after: 1.0,
                timestamp: Utc::now(),
                hash: hash.clone(),
                prev_hash: prev_hash.clone(),
            };
            db.record_gate_decision(&decision).unwrap();
            prev_hash = hash;
        }
        let asc = db.get_gate_decisions_asc(session_id, 100).unwrap();
        assert_eq!(asc.len(), 3);
        assert_eq!(asc[0].tool_name, "Bash");
        assert_eq!(asc[1].tool_name, "Read");
        assert_eq!(asc[2].tool_name, "Edit");
        let mut prev = String::new();
        for d in &asc {
            let expected_input = format!("{}{}{}{}", d.session_id, d.tool_name, d.reason, prev);
            let expected_hash = format!("{:x}", Sha256::digest(expected_input.as_bytes()));
            assert_eq!(d.hash, expected_hash);
            assert_eq!(d.prev_hash, prev);
            prev = d.hash.clone();
        }
        let desc = db.get_gate_decisions(session_id, 100).unwrap();
        assert_eq!(desc[0].tool_name, "Edit");
        assert_eq!(desc[2].tool_name, "Bash");
    }

    fn create_stealable_item(db: &MemoryDb, id: &str, progress: i32, stale_mins: i64) {
        let mut item = test_work_item(id, &format!("Task {id}"));
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item(id, "session-old").unwrap();
        let old_hb = (Utc::now() - chrono::Duration::minutes(stale_mins)).to_rfc3339();
        db.conn
            .execute(
                "UPDATE work_items SET last_heartbeat = ?1, progress = ?2 WHERE id = ?3",
                params![old_hb, progress, id],
            )
            .unwrap();
    }

    #[test]
    fn test_steal_work_item_safe() {
        let db = test_db();
        let mut item = test_work_item("ws-1", "Steal me");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item("ws-1", "old-session").unwrap();
        db.conn
            .execute("UPDATE work_items SET stealable = 1 WHERE id = 'ws-1'", [])
            .unwrap();
        assert!(db.steal_work_item_safe("ws-1", "new-session", 3).unwrap());
        let fetched = db.get_work_item("ws-1").unwrap().unwrap();
        assert_eq!(fetched.claimed_by, Some("new-session".to_string()));
        assert!(!fetched.stealable);
        let count: i32 = db
            .conn
            .query_row(
                "SELECT steal_count FROM work_items WHERE id = 'ws-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_steal_anti_thrashing() {
        let db = test_db();
        let mut item = test_work_item("ws-2", "Anti-thrash");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item("ws-2", "s1").unwrap();
        db.conn
            .execute(
                "UPDATE work_items SET stealable = 1, steal_count = 3 WHERE id = 'ws-2'",
                [],
            )
            .unwrap();
        assert!(!db.steal_work_item_safe("ws-2", "s2", 3).unwrap());
    }

    #[test]
    fn test_claim_load_aware() {
        let db = test_db();
        for i in 0..2 {
            let mut item = test_work_item(&format!("la-{i}"), &format!("Load {i}"));
            item.status = "in_progress".to_string();
            db.create_work_item(&item).unwrap();
            db.claim_work_item(&format!("la-{i}"), "session-a").unwrap();
        }
        let mut item3 = test_work_item("la-2", "Load 2");
        item3.status = "in_progress".to_string();
        db.create_work_item(&item3).unwrap();
        assert!(db
            .claim_work_item_load_aware("la-2", "session-a", 3)
            .unwrap());
        let mut item4 = test_work_item("la-3", "Load 3");
        item4.status = "in_progress".to_string();
        db.create_work_item(&item4).unwrap();
        assert!(!db
            .claim_work_item_load_aware("la-3", "session-a", 3)
            .unwrap());
    }

    #[test]
    fn test_detect_stale_tiered_progress() {
        let db = test_db();
        create_stealable_item(&db, "st-0", 0, 35);
        create_stealable_item(&db, "st-50", 50, 35);
        create_stealable_item(&db, "st-90", 90, 120);
        assert_eq!(db.detect_stale_tiered(30, 3, 10).unwrap(), 1);
        assert!(db.get_work_item("st-0").unwrap().unwrap().stealable);
        assert!(!db.get_work_item("st-50").unwrap().unwrap().stealable);
        assert!(!db.get_work_item("st-90").unwrap().unwrap().stealable);
    }

    #[test]
    fn test_detect_stale_tiered_cooldown() {
        let db = test_db();
        create_stealable_item(&db, "cd-1", 0, 60);
        let now = Utc::now().to_rfc3339();
        db.conn
            .execute(
                "UPDATE work_items SET last_stolen_at = ?1 WHERE id = 'cd-1'",
                params![now],
            )
            .unwrap();
        assert_eq!(db.detect_stale_tiered(30, 3, 10).unwrap(), 0);
    }

    #[test]
    fn test_record_session_effectiveness() {
        let db = test_db();
        let session_id = "eff-sess-1";
        let session = SessionInfo {
            id: session_id.to_string(),
            started_at: Utc::now(),
            ended_at: None,
            cwd: ".".to_string(),
            edits: 0,
            commands: 0,
            summary: None,
            transcript_path: None,
        };
        db.create_session(&session).unwrap();
        db.record_context_injection(session_id, None, "pattern", Some("pat-1"), Some(0.8))
            .unwrap();
        db.record_context_injection(session_id, None, "pattern", Some("pat-2"), Some(0.6))
            .unwrap();
        assert_eq!(
            db.record_session_effectiveness(session_id, "success")
                .unwrap(),
            2
        );
    }

    #[test]
    fn test_recompute_effectiveness_decay() {
        let db = test_db();
        let now = Utc::now().to_rfc3339();
        let old = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        db.conn.execute(
            "INSERT INTO pattern_effectiveness (pattern_id, session_id, outcome, similarity, timestamp) VALUES ('decay-pat', 'sess-a', 'success', 0.9, ?1)",
            params![now],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO pattern_effectiveness (pattern_id, session_id, outcome, similarity, timestamp) VALUES ('decay-pat', 'sess-b', 'failure', 0.9, ?1)",
            params![old],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO patterns_long (id, content, category, usage_count, last_used, effectiveness_score, effectiveness_samples) VALUES ('decay-pat', 'test pattern', 'test', 1, ?1, 0.0, 0)",
            params![now],
        ).unwrap();
        db.recompute_pattern_effectiveness("decay-pat").unwrap();
        let eff = db.get_pattern_effectiveness_score("decay-pat").unwrap();
        assert!(
            eff.score > 0.7,
            "Expected score > 0.7 due to decay, got {}",
            eff.score
        );
        assert_eq!(eff.samples, 2);
    }

    #[test]
    fn test_get_patterns_by_effectiveness() {
        let db = test_db();
        let now = Utc::now().to_rfc3339();
        for (id, content, score, samples) in [
            ("eff-a", "pattern alpha", 0.9, 5),
            ("eff-b", "pattern beta", 0.3, 4),
            ("eff-c", "pattern gamma", 0.6, 3),
        ] {
            db.conn.execute(
                "INSERT INTO patterns_long (id, content, category, usage_count, last_used, effectiveness_score, effectiveness_samples) VALUES (?1, ?2, 'test', 1, ?3, ?4, ?5)",
                params![id, content, now, score, samples],
            ).unwrap();
        }
        let asc = db.get_patterns_by_effectiveness(10, true).unwrap();
        assert_eq!(asc[0].0, "eff-b");
        assert_eq!(asc[2].0, "eff-a");
        let desc = db.get_patterns_by_effectiveness(10, false).unwrap();
        assert_eq!(desc[0].0, "eff-a");
        assert_eq!(desc[2].0, "eff-b");
        assert_eq!(db.get_patterns_by_effectiveness(2, false).unwrap().len(), 2);
    }

    // ── MCP-style roundtrip tests ──

    #[test]
    fn test_memory_set_get_roundtrip() {
        let db = test_db();
        db.kv_set("project", "flowforge", "default").unwrap();
        db.kv_set("version", "1.0", "default").unwrap();
        assert_eq!(
            db.kv_get("project", "default").unwrap(),
            Some("flowforge".to_string())
        );
        assert_eq!(
            db.kv_get("version", "default").unwrap(),
            Some("1.0".to_string())
        );
    }

    #[test]
    fn test_memory_set_overwrite() {
        let db = test_db();
        db.kv_set("key", "v1", "default").unwrap();
        db.kv_set("key", "v2", "default").unwrap();
        assert_eq!(db.kv_get("key", "default").unwrap(), Some("v2".to_string()));
    }

    #[test]
    fn test_memory_namespace_isolation() {
        let db = test_db();
        db.kv_set("key", "val-a", "ns-a").unwrap();
        db.kv_set("key", "val-b", "ns-b").unwrap();
        assert_eq!(db.kv_get("key", "ns-a").unwrap(), Some("val-a".to_string()));
        assert_eq!(db.kv_get("key", "ns-b").unwrap(), Some("val-b".to_string()));
    }

    #[test]
    fn test_memory_list_namespace() {
        let db = test_db();
        db.kv_set("alpha", "1", "test-ns").unwrap();
        db.kv_set("beta", "2", "test-ns").unwrap();
        db.kv_set("gamma", "3", "other-ns").unwrap();
        let items = db.kv_list("test-ns").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, "alpha");
        assert_eq!(items[1].0, "beta");
    }

    #[test]
    fn test_memory_search() {
        let db = test_db();
        db.kv_set("rust-version", "1.86", "default").unwrap();
        db.kv_set("python-version", "3.11", "default").unwrap();
        db.kv_set("node-version", "20", "default").unwrap();
        let results = db.kv_search("version", 10).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_memory_count() {
        let db = test_db();
        assert_eq!(db.count_kv().unwrap(), 0);
        db.kv_set("a", "1", "default").unwrap();
        db.kv_set("b", "2", "default").unwrap();
        assert_eq!(db.count_kv().unwrap(), 2);
    }

    #[test]
    fn test_work_create_list_roundtrip() {
        let db = test_db();
        db.create_work_item(&test_work_item("rt-1", "Build feature"))
            .unwrap();
        db.create_work_item(&test_work_item("rt-2", "Fix bug"))
            .unwrap();
        let items = db.list_work_items(&WorkFilter::default()).unwrap();
        assert_eq!(items.len(), 2);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"Build feature"));
        assert!(titles.contains(&"Fix bug"));
    }

    #[test]
    fn test_checkpoint_create_get_roundtrip() {
        use flowforge_core::Checkpoint;
        let db = test_db();
        let session = SessionInfo {
            id: "cp-sess".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            cwd: "/tmp".to_string(),
            edits: 0,
            commands: 0,
            summary: None,
            transcript_path: None,
        };
        db.create_session(&session).unwrap();
        let cp = Checkpoint {
            id: "cp-1".to_string(),
            session_id: "cp-sess".to_string(),
            name: "before-refactor".to_string(),
            message_index: 5,
            description: Some("Save point".to_string()),
            git_ref: Some("abc123".to_string()),
            created_at: Utc::now(),
            metadata: None,
        };
        db.create_checkpoint(&cp).unwrap();
        let fetched = db.get_checkpoint("cp-1").unwrap().unwrap();
        assert_eq!(fetched.name, "before-refactor");
        assert_eq!(fetched.message_index, 5);
        assert_eq!(fetched.git_ref, Some("abc123".to_string()));
    }

    #[test]
    fn test_checkpoint_list_by_session() {
        use flowforge_core::Checkpoint;
        let db = test_db();
        let session = SessionInfo {
            id: "cp-list-sess".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            cwd: "/tmp".to_string(),
            edits: 0,
            commands: 0,
            summary: None,
            transcript_path: None,
        };
        db.create_session(&session).unwrap();
        for (i, name) in ["start", "middle", "end"].iter().enumerate() {
            let cp = Checkpoint {
                id: format!("cp-l-{i}"),
                session_id: "cp-list-sess".to_string(),
                name: name.to_string(),
                message_index: (i * 10) as u32,
                description: None,
                git_ref: None,
                created_at: Utc::now(),
                metadata: None,
            };
            db.create_checkpoint(&cp).unwrap();
        }
        let cps = db.list_checkpoints("cp-list-sess").unwrap();
        assert_eq!(cps.len(), 3);
        assert_eq!(cps[0].name, "start");
        assert_eq!(cps[2].name, "end");
    }

    #[test]
    fn test_checkpoint_by_name() {
        use flowforge_core::Checkpoint;
        let db = test_db();
        let session = SessionInfo {
            id: "cp-name-sess".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            cwd: "/tmp".to_string(),
            edits: 0,
            commands: 0,
            summary: None,
            transcript_path: None,
        };
        db.create_session(&session).unwrap();
        let cp = Checkpoint {
            id: "cp-n-1".to_string(),
            session_id: "cp-name-sess".to_string(),
            name: "unique-name".to_string(),
            message_index: 1,
            description: None,
            git_ref: None,
            created_at: Utc::now(),
            metadata: None,
        };
        db.create_checkpoint(&cp).unwrap();
        let found = db
            .get_checkpoint_by_name("cp-name-sess", "unique-name")
            .unwrap();
        assert!(found.is_some());
        let not_found = db
            .get_checkpoint_by_name("cp-name-sess", "missing")
            .unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_checkpoint_delete() {
        use flowforge_core::Checkpoint;
        let db = test_db();
        let cp = Checkpoint {
            id: "cp-del".to_string(),
            session_id: "s".to_string(),
            name: "delete-me".to_string(),
            message_index: 0,
            description: None,
            git_ref: None,
            created_at: Utc::now(),
            metadata: None,
        };
        db.create_checkpoint(&cp).unwrap();
        assert!(db.get_checkpoint("cp-del").unwrap().is_some());
        db.delete_checkpoint("cp-del").unwrap();
        assert!(db.get_checkpoint("cp-del").unwrap().is_none());
    }

    // ── Work-stealing edge cases ──

    #[test]
    fn test_claim_already_claimed_item_fails() {
        let db = test_db();
        let mut item = test_work_item("ws-dup", "Claimed item");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        assert!(db.claim_work_item("ws-dup", "session-1").unwrap());
        // second claim by different session should fail (item is not stealable)
        assert!(!db.claim_work_item("ws-dup", "session-2").unwrap());
    }

    #[test]
    fn test_release_then_reclaim() {
        let db = test_db();
        let mut item = test_work_item("ws-rel", "Release me");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item("ws-rel", "s1").unwrap();
        db.release_work_item("ws-rel").unwrap();
        let released = db.get_work_item("ws-rel").unwrap().unwrap();
        assert!(released.claimed_by.is_none());
        // now another session can claim it
        assert!(db.claim_work_item("ws-rel", "s2").unwrap());
        assert_eq!(
            db.get_work_item("ws-rel").unwrap().unwrap().claimed_by,
            Some("s2".to_string())
        );
    }

    #[test]
    fn test_heartbeat_updates_all_claimed_items() {
        let db = test_db();
        for i in 0..3 {
            let mut item = test_work_item(&format!("hb-{i}"), &format!("HB task {i}"));
            item.status = "in_progress".to_string();
            db.create_work_item(&item).unwrap();
            db.claim_work_item(&format!("hb-{i}"), "my-session")
                .unwrap();
        }
        let count = db.update_heartbeat("my-session").unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_progress_update() {
        let db = test_db();
        db.create_work_item(&test_work_item("prog-1", "Progress"))
            .unwrap();
        assert_eq!(db.get_work_item("prog-1").unwrap().unwrap().progress, 0);
        db.update_progress("prog-1", 75).unwrap();
        assert_eq!(db.get_work_item("prog-1").unwrap().unwrap().progress, 75);
    }

    #[test]
    fn test_steal_reclaim_cycle() {
        let db = test_db();
        let mut item = test_work_item("ws-cycle", "Steal cycle");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item("ws-cycle", "s1").unwrap();
        // make stealable
        db.conn
            .execute(
                "UPDATE work_items SET stealable = 1 WHERE id = 'ws-cycle'",
                [],
            )
            .unwrap();
        // steal
        assert!(db.steal_work_item("ws-cycle", "s2").unwrap());
        let stolen = db.get_work_item("ws-cycle").unwrap().unwrap();
        assert_eq!(stolen.claimed_by, Some("s2".to_string()));
        assert!(!stolen.stealable);
        // release and reclaim by original
        db.release_work_item("ws-cycle").unwrap();
        assert!(db.claim_work_item("ws-cycle", "s1").unwrap());
    }

    #[test]
    fn test_mark_stale_items_respects_min_progress() {
        let db = test_db();
        // item with high progress should NOT be marked stealable
        let mut item = test_work_item("stale-hp", "High progress");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item("stale-hp", "old-sess").unwrap();
        db.update_progress("stale-hp", 50).unwrap();
        let old_hb = (Utc::now() - chrono::Duration::minutes(60)).to_rfc3339();
        db.conn
            .execute(
                "UPDATE work_items SET last_heartbeat = ?1 WHERE id = 'stale-hp'",
                params![old_hb],
            )
            .unwrap();
        let count = db.mark_stale_items_stealable(30, 75).unwrap();
        assert_eq!(count, 1); // 50 < 75, so it IS marked stealable
                              // but if min_progress = 30, it should NOT be marked
        db.conn
            .execute(
                "UPDATE work_items SET stealable = 0 WHERE id = 'stale-hp'",
                [],
            )
            .unwrap();
        let count2 = db.mark_stale_items_stealable(30, 30).unwrap();
        assert_eq!(count2, 0); // 50 >= 30, so not marked
    }

    #[test]
    fn test_auto_release_abandoned() {
        let db = test_db();
        let mut item = test_work_item("abandon-1", "Abandoned");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item("abandon-1", "old-sess").unwrap();
        let very_old = (Utc::now() - chrono::Duration::minutes(120)).to_rfc3339();
        db.conn
            .execute(
                "UPDATE work_items SET last_heartbeat = ?1 WHERE id = 'abandon-1'",
                params![very_old],
            )
            .unwrap();
        let released = db.auto_release_abandoned(60).unwrap();
        assert_eq!(released, 1);
        let item = db.get_work_item("abandon-1").unwrap().unwrap();
        assert!(item.claimed_by.is_none());
        assert_eq!(item.status, "pending");
    }

    #[test]
    fn test_get_session_load() {
        let db = test_db();
        // no items yet
        assert_eq!(db.get_session_load("empty-sess").unwrap(), 0);
        for i in 0..3 {
            let mut item = test_work_item(&format!("load-{i}"), &format!("Load {i}"));
            item.status = "in_progress".to_string();
            db.create_work_item(&item).unwrap();
            db.claim_work_item(&format!("load-{i}"), "busy-sess")
                .unwrap();
        }
        assert_eq!(db.get_session_load("busy-sess").unwrap(), 3);
    }

    #[test]
    fn test_get_stealable_items() {
        let db = test_db();
        let mut item = test_work_item("stealable-1", "Ready to steal");
        item.status = "in_progress".to_string();
        db.create_work_item(&item).unwrap();
        db.claim_work_item("stealable-1", "old-sess").unwrap();
        db.conn
            .execute(
                "UPDATE work_items SET stealable = 1 WHERE id = 'stealable-1'",
                [],
            )
            .unwrap();
        let stealable = db.get_stealable_items(10).unwrap();
        assert_eq!(stealable.len(), 1);
        assert_eq!(stealable[0].id, "stealable-1");
    }

    // ── Effectiveness tracking ──

    #[test]
    fn test_record_and_query_context_injection() {
        let db = test_db();
        let session = SessionInfo {
            id: "inj-sess".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            cwd: ".".to_string(),
            edits: 0,
            commands: 0,
            summary: None,
            transcript_path: None,
        };
        db.create_session(&session).unwrap();
        let id1 = db
            .record_context_injection("inj-sess", None, "pattern", Some("pat-1"), Some(0.9))
            .unwrap();
        let id2 = db
            .record_context_injection("inj-sess", None, "trajectory", Some("traj-1"), Some(0.7))
            .unwrap();
        assert!(id1 > 0);
        assert!(id2 > id1);
        let injections = db.get_injections_for_session("inj-sess").unwrap();
        assert_eq!(injections.len(), 2);
        assert_eq!(injections[0].injection_type, "pattern");
        assert_eq!(injections[1].injection_type, "trajectory");
    }

    #[test]
    fn test_rate_context_injection() {
        let db = test_db();
        let id = db
            .record_context_injection("rate-sess", None, "pattern", Some("p"), Some(0.5))
            .unwrap();
        db.rate_context_injection(id, "correlated_success").unwrap();
        let injections = db.get_injections_for_session("rate-sess").unwrap();
        // effectiveness column updated (not directly queryable via struct but DB round-trip works)
        assert_eq!(injections.len(), 1);
    }

    #[test]
    fn test_rate_session_injections() {
        let db = test_db();
        db.record_context_injection("batch-sess", None, "pattern", Some("p1"), Some(0.8))
            .unwrap();
        db.record_context_injection("batch-sess", None, "pattern", Some("p2"), Some(0.6))
            .unwrap();
        let rated = db
            .rate_session_injections("batch-sess", "correlated_success")
            .unwrap();
        assert_eq!(rated, 2);
        // rating twice should not re-rate already rated ones
        let re_rated = db
            .rate_session_injections("batch-sess", "correlated_failure")
            .unwrap();
        assert_eq!(re_rated, 0);
    }

    #[test]
    fn test_record_pattern_effectiveness() {
        let db = test_db();
        db.record_pattern_effectiveness("pat-eff-1", "sess-1", "success", 0.9)
            .unwrap();
        db.record_pattern_effectiveness("pat-eff-1", "sess-2", "failure", 0.7)
            .unwrap();
        // no crash, just verify it records
        let now = Utc::now().to_rfc3339();
        db.conn.execute(
            "INSERT INTO patterns_long (id, content, category, usage_count, last_used, effectiveness_score, effectiveness_samples) VALUES ('pat-eff-1', 'test', 'test', 1, ?1, 0.0, 0)",
            params![now],
        ).unwrap();
        db.recompute_pattern_effectiveness("pat-eff-1").unwrap();
        let eff = db.get_pattern_effectiveness_score("pat-eff-1").unwrap();
        assert_eq!(eff.samples, 2);
        assert!(eff.score > 0.0);
    }

    // ── Pattern lifecycle ──

    #[test]
    fn test_pattern_short_create_search() {
        use flowforge_core::ShortTermPattern;
        let db = test_db();
        let pat = ShortTermPattern {
            id: "sp-1".to_string(),
            content: "Always run cargo test before commit".to_string(),
            category: "workflow".to_string(),
            confidence: 0.7,
            usage_count: 1,
            created_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_short(&pat).unwrap();
        let found = db.search_patterns_short("cargo test", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "sp-1");
    }

    #[test]
    fn test_pattern_short_usage_increment() {
        use flowforge_core::ShortTermPattern;
        let db = test_db();
        let pat = ShortTermPattern {
            id: "sp-use".to_string(),
            content: "test pattern".to_string(),
            category: "test".to_string(),
            confidence: 0.5,
            usage_count: 0,
            created_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_short(&pat).unwrap();
        db.update_pattern_short_usage("sp-use").unwrap();
        db.update_pattern_short_usage("sp-use").unwrap();
        let updated = db.get_pattern_short("sp-use").unwrap().unwrap();
        assert_eq!(updated.usage_count, 2);
        assert!(updated.confidence > 0.5); // confidence increases with usage
    }

    #[test]
    fn test_pattern_promote_short_to_long() {
        use flowforge_core::{LongTermPattern, ShortTermPattern};
        let db = test_db();
        let pat = ShortTermPattern {
            id: "promote-1".to_string(),
            content: "Promote me".to_string(),
            category: "test".to_string(),
            confidence: 0.8,
            usage_count: 5,
            created_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_short(&pat).unwrap();
        // Simulate promotion: create long-term version and delete short-term
        let long = LongTermPattern {
            id: pat.id.clone(),
            content: pat.content.clone(),
            category: pat.category.clone(),
            confidence: pat.confidence,
            usage_count: pat.usage_count,
            success_count: 0,
            failure_count: 0,
            created_at: pat.created_at,
            promoted_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_long(&long).unwrap();
        db.delete_pattern_short("promote-1").unwrap();
        assert!(db.get_pattern_short("promote-1").unwrap().is_none());
        let promoted = db.get_pattern_long("promote-1").unwrap().unwrap();
        assert_eq!(promoted.content, "Promote me");
        assert_eq!(promoted.usage_count, 5);
    }

    #[test]
    fn test_pattern_long_feedback() {
        use flowforge_core::LongTermPattern;
        let db = test_db();
        let pat = LongTermPattern {
            id: "fb-1".to_string(),
            content: "Feedback test".to_string(),
            category: "test".to_string(),
            confidence: 0.5,
            usage_count: 1,
            success_count: 0,
            failure_count: 0,
            created_at: Utc::now(),
            promoted_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_long(&pat).unwrap();
        db.update_pattern_long_feedback("fb-1", true).unwrap();
        db.update_pattern_long_feedback("fb-1", true).unwrap();
        db.update_pattern_long_feedback("fb-1", false).unwrap();
        let updated = db.get_pattern_long("fb-1").unwrap().unwrap();
        assert_eq!(updated.success_count, 2);
        assert_eq!(updated.failure_count, 1);
    }

    #[test]
    fn test_pattern_search_long() {
        use flowforge_core::LongTermPattern;
        let db = test_db();
        let pat = LongTermPattern {
            id: "sl-1".to_string(),
            content: "Use clippy for linting".to_string(),
            category: "quality".to_string(),
            confidence: 0.9,
            usage_count: 10,
            success_count: 8,
            failure_count: 2,
            created_at: Utc::now(),
            promoted_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_long(&pat).unwrap();
        let found = db.search_patterns_long("clippy", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].content, "Use clippy for linting");
        let not_found = db.search_patterns_long("nonexistent", 10).unwrap();
        assert!(not_found.is_empty());
    }

    #[test]
    fn test_pattern_count() {
        use flowforge_core::{LongTermPattern, ShortTermPattern};
        let db = test_db();
        let sp = ShortTermPattern {
            id: "cnt-s".to_string(),
            content: "short".to_string(),
            category: "t".to_string(),
            confidence: 0.5,
            usage_count: 0,
            created_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_short(&sp).unwrap();
        let lp = LongTermPattern {
            id: "cnt-l".to_string(),
            content: "long".to_string(),
            category: "t".to_string(),
            confidence: 0.5,
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            created_at: Utc::now(),
            promoted_at: Utc::now(),
            last_used: Utc::now(),
            embedding_id: None,
        };
        db.store_pattern_long(&lp).unwrap();
        assert_eq!(db.count_patterns_short().unwrap(), 1);
        assert_eq!(db.count_patterns_long().unwrap(), 1);
        assert_eq!(db.count_patterns().unwrap(), 2);
    }

    // ── Trajectory tests ──

    #[test]
    fn test_trajectory_create_get_roundtrip() {
        use flowforge_core::trajectory::{Trajectory, TrajectoryStatus};
        let db = test_db();
        let traj = Trajectory {
            id: "traj-1".to_string(),
            session_id: "sess-1".to_string(),
            work_item_id: None,
            agent_name: Some("coder".to_string()),
            task_description: Some("Fix the bug".to_string()),
            status: TrajectoryStatus::Recording,
            started_at: Utc::now(),
            ended_at: None,
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj).unwrap();
        let fetched = db.get_trajectory("traj-1").unwrap().unwrap();
        assert_eq!(fetched.session_id, "sess-1");
        assert_eq!(fetched.agent_name, Some("coder".to_string()));
        assert_eq!(fetched.status, TrajectoryStatus::Recording);
    }

    #[test]
    fn test_trajectory_steps_recording() {
        use flowforge_core::trajectory::{StepOutcome, Trajectory, TrajectoryStatus};
        let db = test_db();
        let traj = Trajectory {
            id: "traj-steps".to_string(),
            session_id: "sess-2".to_string(),
            work_item_id: None,
            agent_name: None,
            task_description: None,
            status: TrajectoryStatus::Recording,
            started_at: Utc::now(),
            ended_at: None,
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj).unwrap();
        db.record_trajectory_step("traj-steps", "Read", None, StepOutcome::Success, Some(100))
            .unwrap();
        db.record_trajectory_step(
            "traj-steps",
            "Edit",
            Some("abc"),
            StepOutcome::Success,
            Some(200),
        )
        .unwrap();
        db.record_trajectory_step("traj-steps", "Bash", None, StepOutcome::Failure, Some(50))
            .unwrap();
        let steps = db.get_trajectory_steps("traj-steps").unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].tool_name, "Read");
        assert_eq!(steps[0].step_index, 0);
        assert_eq!(steps[1].step_index, 1);
        assert_eq!(steps[2].outcome, StepOutcome::Failure);
    }

    #[test]
    fn test_trajectory_success_ratio() {
        use flowforge_core::trajectory::{StepOutcome, Trajectory, TrajectoryStatus};
        let db = test_db();
        let traj = Trajectory {
            id: "traj-ratio".to_string(),
            session_id: "s".to_string(),
            work_item_id: None,
            agent_name: None,
            task_description: None,
            status: TrajectoryStatus::Recording,
            started_at: Utc::now(),
            ended_at: None,
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj).unwrap();
        for outcome in [
            StepOutcome::Success,
            StepOutcome::Success,
            StepOutcome::Failure,
            StepOutcome::Success,
        ] {
            db.record_trajectory_step("traj-ratio", "Test", None, outcome, None)
                .unwrap();
        }
        let ratio = db.trajectory_success_ratio("traj-ratio").unwrap();
        assert!((ratio - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_trajectory_tool_sequence() {
        use flowforge_core::trajectory::{StepOutcome, Trajectory, TrajectoryStatus};
        let db = test_db();
        let traj = Trajectory {
            id: "traj-seq".to_string(),
            session_id: "s".to_string(),
            work_item_id: None,
            agent_name: None,
            task_description: None,
            status: TrajectoryStatus::Recording,
            started_at: Utc::now(),
            ended_at: None,
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj).unwrap();
        for tool in ["Read", "Grep", "Edit", "Bash"] {
            db.record_trajectory_step("traj-seq", tool, None, StepOutcome::Success, None)
                .unwrap();
        }
        let seq = db.trajectory_tool_sequence("traj-seq").unwrap();
        assert_eq!(seq, vec!["Read", "Grep", "Edit", "Bash"]);
    }

    #[test]
    fn test_trajectory_end_and_judge() {
        use flowforge_core::trajectory::{Trajectory, TrajectoryStatus, TrajectoryVerdict};
        let db = test_db();
        let traj = Trajectory {
            id: "traj-judge".to_string(),
            session_id: "s".to_string(),
            work_item_id: None,
            agent_name: None,
            task_description: None,
            status: TrajectoryStatus::Recording,
            started_at: Utc::now(),
            ended_at: None,
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj).unwrap();
        db.end_trajectory("traj-judge", TrajectoryStatus::Completed)
            .unwrap();
        let ended = db.get_trajectory("traj-judge").unwrap().unwrap();
        assert_eq!(ended.status, TrajectoryStatus::Completed);
        assert!(ended.ended_at.is_some());
        db.judge_trajectory("traj-judge", TrajectoryVerdict::Success, 0.95)
            .unwrap();
        let judged = db.get_trajectory("traj-judge").unwrap().unwrap();
        assert_eq!(judged.status, TrajectoryStatus::Judged);
        assert_eq!(judged.verdict, Some(TrajectoryVerdict::Success));
        assert_eq!(judged.confidence, Some(0.95));
    }

    #[test]
    fn test_trajectory_active_for_session() {
        use flowforge_core::trajectory::{Trajectory, TrajectoryStatus};
        let db = test_db();
        let traj1 = Trajectory {
            id: "traj-active-1".to_string(),
            session_id: "active-sess".to_string(),
            work_item_id: None,
            agent_name: None,
            task_description: None,
            status: TrajectoryStatus::Completed,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj1).unwrap();
        let traj2 = Trajectory {
            id: "traj-active-2".to_string(),
            session_id: "active-sess".to_string(),
            work_item_id: None,
            agent_name: None,
            task_description: None,
            status: TrajectoryStatus::Recording,
            started_at: Utc::now(),
            ended_at: None,
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj2).unwrap();
        let active = db.get_active_trajectory("active-sess").unwrap().unwrap();
        assert_eq!(active.id, "traj-active-2");
    }

    #[test]
    fn test_trajectory_list_with_filters() {
        use flowforge_core::trajectory::{Trajectory, TrajectoryStatus};
        let db = test_db();
        for (id, sid, status) in [
            ("tl-1", "s1", TrajectoryStatus::Recording),
            ("tl-2", "s1", TrajectoryStatus::Completed),
            ("tl-3", "s2", TrajectoryStatus::Recording),
        ] {
            let traj = Trajectory {
                id: id.to_string(),
                session_id: sid.to_string(),
                work_item_id: None,
                agent_name: None,
                task_description: None,
                status,
                started_at: Utc::now(),
                ended_at: None,
                verdict: None,
                confidence: None,
                metadata: None,
                embedding_id: None,
            };
            db.create_trajectory(&traj).unwrap();
        }
        let all = db.list_trajectories(None, None, 100).unwrap();
        assert_eq!(all.len(), 3);
        let s1_only = db.list_trajectories(Some("s1"), None, 100).unwrap();
        assert_eq!(s1_only.len(), 2);
        let recording = db.list_trajectories(None, Some("recording"), 100).unwrap();
        assert_eq!(recording.len(), 2);
    }

    #[test]
    fn test_trajectory_link_work_item() {
        use flowforge_core::trajectory::{Trajectory, TrajectoryStatus};
        let db = test_db();
        let traj = Trajectory {
            id: "traj-link".to_string(),
            session_id: "s".to_string(),
            work_item_id: None,
            agent_name: None,
            task_description: None,
            status: TrajectoryStatus::Recording,
            started_at: Utc::now(),
            ended_at: None,
            verdict: None,
            confidence: None,
            metadata: None,
            embedding_id: None,
        };
        db.create_trajectory(&traj).unwrap();
        db.link_trajectory_work_item("traj-link", "wi-123").unwrap();
        let linked = db.get_trajectory("traj-link").unwrap().unwrap();
        assert_eq!(linked.work_item_id, Some("wi-123".to_string()));
    }

    // ── Trust score tests ──

    #[test]
    fn test_trust_score_lifecycle() {
        let db = test_db();
        db.create_trust_score("trust-sess", 0.5).unwrap();
        let score = db.get_trust_score("trust-sess").unwrap().unwrap();
        assert_eq!(score.score, 0.5);
        assert_eq!(score.total_checks, 0);
        db.update_trust_score("trust-sess", &GateAction::Allow, 0.05)
            .unwrap();
        db.update_trust_score("trust-sess", &GateAction::Ask, -0.02)
            .unwrap();
        db.update_trust_score("trust-sess", &GateAction::Deny, -0.1)
            .unwrap();
        let updated = db.get_trust_score("trust-sess").unwrap().unwrap();
        assert_eq!(updated.total_checks, 3);
        assert_eq!(updated.allows, 1);
        assert_eq!(updated.asks, 1);
        assert_eq!(updated.denials, 1);
        let expected_score = 0.5 + 0.05 - 0.02 - 0.1;
        assert!((updated.score - expected_score).abs() < 0.001);
    }

    #[test]
    fn test_trust_score_clamps_to_range() {
        let db = test_db();
        db.create_trust_score("clamp-sess", 0.9).unwrap();
        db.update_trust_score("clamp-sess", &GateAction::Allow, 0.5)
            .unwrap(); // would exceed 1.0
        let score = db.get_trust_score("clamp-sess").unwrap().unwrap();
        assert!(score.score <= 1.0);
        db.create_trust_score("clamp-low", 0.1).unwrap();
        db.update_trust_score("clamp-low", &GateAction::Deny, -0.5)
            .unwrap(); // would go below 0.0
        let low = db.get_trust_score("clamp-low").unwrap().unwrap();
        assert!(low.score >= 0.0);
    }

    // ── Meta operations ──

    #[test]
    fn test_meta_get_set() {
        let db = test_db();
        assert!(db.get_meta("missing").unwrap().is_none());
        db.set_meta("version", "3").unwrap();
        assert_eq!(db.get_meta("version").unwrap(), Some("3".to_string()));
        db.set_meta("version", "4").unwrap();
        assert_eq!(db.get_meta("version").unwrap(), Some("4".to_string()));
    }

    // ── Schema / infrastructure tests ──

    #[test]
    fn test_schema_version_is_stamped() {
        let db = test_db();
        let version = db.get_meta("schema_version").unwrap().unwrap();
        assert_eq!(version, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn test_vector_blob_roundtrip() {
        let original = vec![1.0f32, 2.5, -3.7, 0.0, 42.42];
        let blob = vector_to_blob(&original);
        let restored = blob_to_vector(&blob);
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_work_item_filter_by_type() {
        let db = test_db();
        let mut bug = test_work_item("type-bug", "A bug");
        bug.item_type = "bug".to_string();
        db.create_work_item(&bug).unwrap();
        db.create_work_item(&test_work_item("type-task", "A task"))
            .unwrap();
        let bugs = db
            .list_work_items(&WorkFilter {
                item_type: Some("bug".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].id, "type-bug");
    }

    #[test]
    fn test_work_item_filter_by_assignee() {
        let db = test_db();
        let mut item = test_work_item("assign-1", "Assigned task");
        item.assignee = Some("agent:coder".to_string());
        db.create_work_item(&item).unwrap();
        db.create_work_item(&test_work_item("assign-2", "Unassigned"))
            .unwrap();
        let assigned = db
            .list_work_items(&WorkFilter {
                assignee: Some("agent:coder".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(assigned.len(), 1);
    }

    #[test]
    fn test_work_config_roundtrip() {
        let db = test_db();
        assert!(db.get_work_config("backend").unwrap().is_none());
        db.set_work_config("backend", "kanbus").unwrap();
        assert_eq!(
            db.get_work_config("backend").unwrap(),
            Some("kanbus".to_string())
        );
    }

    // ── test_helpers module tests ──

    #[test]
    fn test_seeded_db_has_data() {
        use crate::test_helpers;
        let db = test_helpers::seeded_db();
        let session = db.get_current_session().unwrap();
        assert!(session.is_some());
        assert_eq!(session.unwrap().id, "test-session");
        let items = db.list_work_items(&WorkFilter::default()).unwrap();
        assert_eq!(items.len(), 2);
        let patterns = db.get_all_patterns_short().unwrap();
        assert_eq!(patterns.len(), 2);
    }

    #[test]
    fn test_helper_work_item_defaults() {
        use crate::test_helpers;
        let item = test_helpers::work_item("test-id", "Test Title");
        assert_eq!(item.id, "test-id");
        assert_eq!(item.title, "Test Title");
        assert_eq!(item.status, "pending");
        assert_eq!(item.backend, "flowforge");
        assert_eq!(item.priority, 2);
        assert!(!item.stealable);
    }
}
